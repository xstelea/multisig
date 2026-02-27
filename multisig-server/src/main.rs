mod gateway;
mod manifest_analyzer;
mod proposal_store;
mod signature_collector;
mod transaction_builder;
mod validity_monitor;

use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::Method,
    routing::{get, post},
    Json, Router,
};
use sqlx::postgres::PgPoolOptions;
use tower_http::cors::{Any, CorsLayer};
use tracing_subscriber::EnvFilter;

use radix_common::address::AddressBech32Encoder;
use radix_common::network::NetworkDefinition;
use radix_common::prelude::{ComponentAddress, Ed25519PrivateKey};

use crate::gateway::GatewayClient;
use crate::proposal_store::{CreateProposal, Proposal, ProposalStatus, ProposalStore};
use crate::signature_collector::{SignatureCollector, SignatureStatus};
use crate::transaction_builder::StoredSignature;

#[derive(Clone)]
pub struct AppState {
    pub proposal_store: Arc<ProposalStore>,
    pub signature_collector: Arc<SignatureCollector>,
    pub gateway: Arc<GatewayClient>,
    pub network_id: u8,
    /// Raw bytes of the server's fee-payer Ed25519 private key.
    pub fee_payer_key_bytes: [u8; 32],
    /// Bech32-encoded preallocated account address for the fee payer (for logging/recording).
    pub fee_payer_account: String,
}

#[derive(serde::Serialize)]
struct HealthResponse {
    status: &'static str,
}

#[derive(serde::Serialize)]
struct ErrorResponse {
    error: String,
}

/// Shorthand for building a JSON error response tuple.
fn err_response(
    status: axum::http::StatusCode,
    msg: String,
) -> (axum::http::StatusCode, Json<ErrorResponse>) {
    (status, Json(ErrorResponse { error: msg }))
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

// --- Proposal endpoints ---

#[derive(serde::Deserialize)]
struct CreateProposalRequest {
    manifest_text: String,
    expiry_epoch: u64,
}

async fn create_proposal(
    State(state): State<AppState>,
    Json(req): Json<CreateProposalRequest>,
) -> Result<Json<Proposal>, (axum::http::StatusCode, Json<ErrorResponse>)> {
    // Compile manifest first — used for both analysis and subintent building
    let compiled_manifest =
        transaction_builder::compile_subintent_manifest(&req.manifest_text, state.network_id)
            .map_err(|e| {
                tracing::error!("Failed to compile manifest: {e}");
                err_response(
                    axum::http::StatusCode::BAD_REQUEST,
                    format!("Failed to compile manifest: {e}"),
                )
            })?;

    // Extract accounts that need authorization from the manifest
    let network_def = match state.network_id {
        0x01 => NetworkDefinition::mainnet(),
        _ => NetworkDefinition::stokenet(),
    };
    let auth_accounts =
        manifest_analyzer::extract_accounts_requiring_auth(&compiled_manifest, &network_def)
            .map_err(|e| {
                tracing::error!("Failed to analyze manifest: {e}");
                err_response(
                    axum::http::StatusCode::BAD_REQUEST,
                    format!("Failed to analyze manifest: {e}"),
                )
            })?;

    // Query access rules for each auth-requiring account and keep those with
    // non-trivial (multi-signer) rules.
    let mut multisig_accounts = Vec::new();
    for account in &auth_accounts {
        let access_rule = state.gateway.read_access_rule(account).await.map_err(|e| {
            tracing::error!("Failed to read access rule for {account}: {e}");
            err_response(
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to read access rule for {account}: {e}"),
            )
        })?;
        if access_rule.signers.len() > 1 || access_rule.threshold > 1 {
            multisig_accounts.push(account.clone());
        }
    }

    let multisig_account = match multisig_accounts.len() {
        0 => {
            return Err(err_response(
                axum::http::StatusCode::BAD_REQUEST,
                "No multisig accounts found in manifest. The manifest must reference an account with multi-signer access rules.".to_string(),
            ));
        }
        1 => multisig_accounts.into_iter().next().unwrap(),
        _ => {
            return Err(err_response(
                axum::http::StatusCode::BAD_REQUEST,
                format!(
                    "Multiple multisig accounts found in manifest: {}. Only one multisig account per proposal is supported.",
                    multisig_accounts.join(", ")
                ),
            ));
        }
    };

    // Get current epoch to set epoch_min
    let current_epoch = state.gateway.get_current_epoch().await.map_err(|e| {
        tracing::error!("Failed to get current epoch: {e}");
        err_response(
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to get current epoch: {e}"),
        )
    })?;

    let epoch_min = current_epoch;
    let epoch_max = req.expiry_epoch;

    if epoch_max <= epoch_min {
        return Err(err_response(
            axum::http::StatusCode::BAD_REQUEST,
            format!("Expiry epoch ({epoch_max}) must be greater than current epoch ({epoch_min})"),
        ));
    }

    // Build the unsigned subintent from the already-compiled manifest
    let subintent_result = transaction_builder::build_unsigned_subintent_from_compiled(
        compiled_manifest,
        state.network_id,
        epoch_min,
        epoch_max,
    )
    .map_err(|e| {
        tracing::error!("Failed to build subintent: {e}");
        err_response(
            axum::http::StatusCode::BAD_REQUEST,
            format!("Failed to build subintent: {e}"),
        )
    })?;

    // Store the proposal
    let proposal = state
        .proposal_store
        .create(CreateProposal {
            manifest_text: req.manifest_text,
            multisig_account,
            epoch_min: epoch_min as i64,
            epoch_max: epoch_max as i64,
            subintent_hash: subintent_result.subintent_hash,
            intent_discriminator: subintent_result.intent_discriminator as i64,
            min_proposer_timestamp: subintent_result.min_proposer_timestamp,
            max_proposer_timestamp: subintent_result.max_proposer_timestamp,
            partial_transaction_bytes: subintent_result.partial_transaction_bytes,
        })
        .await
        .map_err(|e| {
            tracing::error!("Failed to create proposal: {e}");
            err_response(
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to create proposal: {e}"),
            )
        })?;

    Ok(Json(proposal))
}

async fn list_proposals(
    State(state): State<AppState>,
) -> Result<Json<Vec<Proposal>>, (axum::http::StatusCode, Json<ErrorResponse>)> {
    state.proposal_store.list().await.map(Json).map_err(|e| {
        tracing::error!("Failed to list proposals: {e}");
        err_response(
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to list proposals: {e}"),
        )
    })
}

async fn get_proposal(
    State(state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<Proposal>, (axum::http::StatusCode, Json<ErrorResponse>)> {
    let proposal = state
        .proposal_store
        .get(id)
        .await
        .map_err(|e| {
            tracing::error!("Failed to get proposal: {e}");
            err_response(
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to get proposal: {e}"),
            )
        })?
        .ok_or_else(|| {
            err_response(
                axum::http::StatusCode::NOT_FOUND,
                "Proposal not found".to_string(),
            )
        })?;

    Ok(Json(proposal))
}

// --- Signature endpoints ---

#[derive(serde::Deserialize)]
struct SignProposalRequest {
    signed_partial_transaction_hex: String,
}

async fn sign_proposal(
    State(state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
    Json(req): Json<SignProposalRequest>,
) -> Result<Json<SignatureStatus>, (axum::http::StatusCode, Json<ErrorResponse>)> {
    // Fetch proposal first to get its multisig account and subintent hash
    let proposal = state
        .proposal_store
        .get(id)
        .await
        .map_err(|e| {
            tracing::error!("Failed to get proposal: {e}");
            err_response(
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to get proposal: {e}"),
            )
        })?
        .ok_or_else(|| {
            err_response(
                axum::http::StatusCode::NOT_FOUND,
                "Proposal not found".to_string(),
            )
        })?;

    // Fetch current access rule for validation using proposal's multisig account
    let access_rule = state
        .gateway
        .read_access_rule(&proposal.multisig_account)
        .await
        .map_err(|e| {
            tracing::error!("Failed to read access rule: {e}");
            err_response(
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to read access rule: {e}"),
            )
        })?;

    let expected_hash = proposal.subintent_hash.ok_or_else(|| {
        err_response(
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "Proposal has no subintent hash".to_string(),
        )
    })?;

    state
        .signature_collector
        .add_signature(
            id,
            &req.signed_partial_transaction_hex,
            &access_rule,
            &state.proposal_store,
            &expected_hash,
            state.network_id,
        )
        .await
        .map(Json)
        .map_err(|e| {
            let msg = e.to_string();
            tracing::warn!("Sign proposal failed: {msg}");
            let status = if msg.contains("not found") {
                axum::http::StatusCode::NOT_FOUND
            } else if msg.contains("not in the current access rule")
                || msg.contains("already signed")
                || msg.contains("status")
                || msg.contains("different subintent hash")
            {
                axum::http::StatusCode::BAD_REQUEST
            } else {
                axum::http::StatusCode::INTERNAL_SERVER_ERROR
            };
            err_response(status, msg)
        })
}

async fn get_signature_status(
    State(state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<SignatureStatus>, (axum::http::StatusCode, Json<ErrorResponse>)> {
    let proposal = state
        .proposal_store
        .get(id)
        .await
        .map_err(|e| {
            tracing::error!("Failed to get proposal: {e}");
            err_response(
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to get proposal: {e}"),
            )
        })?
        .ok_or_else(|| {
            err_response(
                axum::http::StatusCode::NOT_FOUND,
                "Proposal not found".to_string(),
            )
        })?;

    let access_rule = state
        .gateway
        .read_access_rule(&proposal.multisig_account)
        .await
        .map_err(|e| {
            tracing::error!("Failed to read access rule: {e}");
            err_response(
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to read access rule: {e}"),
            )
        })?;

    state
        .signature_collector
        .get_signature_status(id, &access_rule)
        .await
        .map(Json)
        .map_err(|e| {
            tracing::error!("Failed to get signature status: {e}");
            err_response(
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to get signature status: {e}"),
            )
        })
}

// --- Submission endpoints ---

#[derive(serde::Serialize)]
struct SubmitProposalResponse {
    status: String,
    tx_id: Option<String>,
    error: Option<String>,
}

async fn submit_proposal(
    State(state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<SubmitProposalResponse>, (axum::http::StatusCode, Json<ErrorResponse>)> {
    // Validate proposal is in Ready state
    let proposal = state
        .proposal_store
        .get(id)
        .await
        .map_err(|e| {
            tracing::error!("Failed to get proposal: {e}");
            err_response(
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to get proposal: {e}"),
            )
        })?
        .ok_or_else(|| {
            err_response(
                axum::http::StatusCode::NOT_FOUND,
                "Proposal not found".to_string(),
            )
        })?;

    if proposal.status != ProposalStatus::Ready {
        return Err(err_response(
            axum::http::StatusCode::BAD_REQUEST,
            format!(
                "Proposal is in {:?} status; must be Ready to submit",
                proposal.status
            ),
        ));
    }

    // Reconstruct the fee payer private key from stored bytes
    let fee_payer_private_key =
        Ed25519PrivateKey::from_bytes(&state.fee_payer_key_bytes).map_err(|e| {
            tracing::error!("Failed to reconstruct fee payer key: {e:?}");
            err_response(
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "Server fee payer key misconfigured".to_string(),
            )
        })?;
    let fee_payer_account =
        ComponentAddress::preallocated_account_from_public_key(&fee_payer_private_key.public_key());

    // Reconstruct the DAO withdrawal signed partial from stored data
    let partial_bytes = state
        .proposal_store
        .get_partial_transaction_bytes(id)
        .await
        .map_err(|e| {
            tracing::error!("Failed to get partial transaction bytes: {e}");
            err_response(
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to get partial transaction bytes: {e}"),
            )
        })?;

    let raw_sigs = state
        .signature_collector
        .get_raw_signatures(id)
        .await
        .map_err(|e| {
            tracing::error!("Failed to get signatures: {e}");
            err_response(
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to get signatures: {e}"),
            )
        })?;

    let stored_sigs: Vec<StoredSignature> = raw_sigs
        .into_iter()
        .map(|(pk, sig)| StoredSignature {
            public_key_hex: pk,
            signature_bytes: sig,
        })
        .collect();

    let withdrawal_signed_partial =
        transaction_builder::reconstruct_signed_partial(&partial_bytes, &stored_sigs).map_err(
            |e| {
                tracing::error!("Failed to reconstruct signed partial: {e}");
                err_response(
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Failed to reconstruct signed partial: {e}"),
                )
            },
        )?;

    // Get current epoch for the main transaction
    let current_epoch = state.gateway.get_current_epoch().await.map_err(|e| {
        tracing::error!("Failed to get current epoch: {e}");
        err_response(
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to get current epoch: {e}"),
        )
    })?;

    // Compose the main transaction (server pays fee via its own account)
    let composed = transaction_builder::compose_main_transaction(
        state.network_id,
        current_epoch,
        &fee_payer_private_key,
        fee_payer_account,
        withdrawal_signed_partial,
    )
    .map_err(|e| {
        tracing::error!("Failed to compose main transaction: {e}");
        err_response(
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to compose main transaction: {e}"),
        )
    })?;

    // Transition Ready → Submitting
    state
        .proposal_store
        .transition_status(id, ProposalStatus::Ready, ProposalStatus::Submitting)
        .await
        .map_err(|e| {
            tracing::error!("Failed to transition to Submitting: {e}");
            err_response(
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to transition to Submitting: {e}"),
            )
        })?;

    // Record submission attempt
    let _ = state
        .proposal_store
        .record_submission_attempt(
            id,
            &state.fee_payer_account,
            Some(&composed.intent_hash),
            "submitting",
            None,
        )
        .await;

    // Submit to Gateway
    let submit_result = state
        .gateway
        .submit_transaction(&composed.notarized_transaction_hex)
        .await;

    match submit_result {
        Ok(duplicate) => {
            if duplicate {
                tracing::warn!("Transaction was a duplicate submission");
            }
            tracing::info!("Transaction submitted: {}", composed.intent_hash);

            // Store the tx_id
            let _ = state
                .proposal_store
                .update_tx_id(id, &composed.intent_hash)
                .await;

            // Poll for commit (max 60 attempts = ~2 minutes)
            match state
                .gateway
                .wait_for_commit(&composed.intent_hash, 60)
                .await
            {
                Ok(_status) => {
                    let _ = state
                        .proposal_store
                        .transition_status(
                            id,
                            ProposalStatus::Submitting,
                            ProposalStatus::Committed,
                        )
                        .await;

                    Ok(Json(SubmitProposalResponse {
                        status: "committed".to_string(),
                        tx_id: Some(composed.intent_hash),
                        error: None,
                    }))
                }
                Err(e) => {
                    let err_msg = e.to_string();
                    let _ = state
                        .proposal_store
                        .transition_status(id, ProposalStatus::Submitting, ProposalStatus::Failed)
                        .await;

                    Ok(Json(SubmitProposalResponse {
                        status: "failed".to_string(),
                        tx_id: Some(composed.intent_hash),
                        error: Some(err_msg),
                    }))
                }
            }
        }
        Err(e) => {
            let err_msg = e.to_string();
            tracing::error!("Submit failed: {err_msg}");

            let _ = state
                .proposal_store
                .transition_status(id, ProposalStatus::Submitting, ProposalStatus::Failed)
                .await;

            Ok(Json(SubmitProposalResponse {
                status: "failed".to_string(),
                tx_id: Some(composed.intent_hash),
                error: Some(err_msg),
            }))
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let port = std::env::var("PORT").unwrap_or_else(|_| "3001".into());
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let gateway_url = std::env::var("GATEWAY_URL")
        .unwrap_or_else(|_| "https://babylon-stokenet-gateway.radixdlt.com".into());
    let frontend_origin =
        std::env::var("FRONTEND_ORIGIN").unwrap_or_else(|_| "http://localhost:3000".into());
    let network_id: u8 = std::env::var("NETWORK_ID")
        .unwrap_or_else(|_| "2".into())
        .parse()
        .expect("NETWORK_ID must be a valid u8");
    let monitor_interval_secs: u64 = std::env::var("MONITOR_INTERVAL_SECS")
        .unwrap_or_else(|_| "30".into())
        .parse()
        .expect("MONITOR_INTERVAL_SECS must be a valid u64");

    // Fee payer key: server pays tx fees so the wallet doesn't need a fee subintent.
    let fee_payer_key_hex = std::env::var("FEE_PAYER_PRIVATE_KEY_HEX")
        .expect("FEE_PAYER_PRIVATE_KEY_HEX must be set (64 hex chars = 32-byte Ed25519 key)");
    let fee_payer_key_bytes_vec =
        hex::decode(&fee_payer_key_hex).expect("FEE_PAYER_PRIVATE_KEY_HEX must be valid hex");
    let fee_payer_key_bytes: [u8; 32] = fee_payer_key_bytes_vec
        .try_into()
        .expect("FEE_PAYER_PRIVATE_KEY_HEX must be exactly 32 bytes (64 hex chars)");
    let fee_payer_private_key =
        Ed25519PrivateKey::from_bytes(&fee_payer_key_bytes).expect("Invalid fee payer private key");
    let fee_payer_account_addr =
        ComponentAddress::preallocated_account_from_public_key(&fee_payer_private_key.public_key());
    let network_def = match network_id {
        0x01 => NetworkDefinition::mainnet(),
        _ => NetworkDefinition::stokenet(),
    };
    let addr_encoder = AddressBech32Encoder::new(&network_def);
    let fee_payer_account = addr_encoder
        .encode(fee_payer_account_addr.as_bytes())
        .expect("Failed to encode fee payer address");
    tracing::info!(
        "Server fee payer account: {} — fund this with XRD on Stokenet",
        fee_payer_account
    );

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;

    sqlx::migrate!("./migrations").run(&pool).await?;
    tracing::info!("Database migrations applied");

    let gateway = Arc::new(GatewayClient::new(gateway_url));
    let proposal_store = Arc::new(ProposalStore::new(pool.clone()));
    let signature_collector = Arc::new(SignatureCollector::new(pool));

    let state = AppState {
        proposal_store,
        signature_collector,
        gateway,
        network_id,
        fee_payer_key_bytes,
        fee_payer_account,
    };

    // Spawn validity monitor background task
    tokio::spawn(validity_monitor::run(
        state.proposal_store.clone(),
        state.gateway.clone(),
        monitor_interval_secs,
    ));
    tracing::info!("Validity monitor started (interval: {monitor_interval_secs}s)");

    let cors = CorsLayer::new()
        .allow_origin(
            frontend_origin
                .parse::<axum::http::HeaderValue>()
                .expect("Invalid FRONTEND_ORIGIN"),
        )
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers(Any);

    let app = Router::new()
        .route("/health", get(health))
        .route("/proposals", post(create_proposal).get(list_proposals))
        .route("/proposals/{id}", get(get_proposal))
        .route("/proposals/{id}/sign", post(sign_proposal))
        .route("/proposals/{id}/signatures", get(get_signature_status))
        .route("/proposals/{id}/submit", post(submit_proposal))
        .layer(cors)
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}")).await?;
    tracing::info!("Listening on 0.0.0.0:{port}");

    axum::serve(listener, app).await?;

    Ok(())
}
