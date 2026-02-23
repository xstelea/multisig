mod gateway;
mod proposal_store;
mod signature_collector;
mod transaction_builder;

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

use crate::gateway::GatewayClient;
use crate::proposal_store::{CreateProposal, Proposal, ProposalStore};
use crate::signature_collector::{SignatureCollector, SignatureStatus};

#[derive(Clone)]
pub struct AppState {
    pub proposal_store: Arc<ProposalStore>,
    pub signature_collector: Arc<SignatureCollector>,
    pub gateway: Arc<GatewayClient>,
    pub multisig_account: String,
    pub network_id: u8,
}

#[derive(serde::Serialize)]
struct HealthResponse {
    status: &'static str,
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

async fn access_rule(
    State(state): State<AppState>,
) -> Result<Json<gateway::AccessRuleInfo>, (axum::http::StatusCode, String)> {
    state
        .gateway
        .read_access_rule(&state.multisig_account)
        .await
        .map(Json)
        .map_err(|e| {
            tracing::error!("Failed to read access rule: {e}");
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to read access rule: {e}"),
            )
        })
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
) -> Result<Json<Proposal>, (axum::http::StatusCode, String)> {
    // Get current epoch to set epoch_min
    let current_epoch = state.gateway.get_current_epoch().await.map_err(|e| {
        tracing::error!("Failed to get current epoch: {e}");
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to get current epoch: {e}"),
        )
    })?;

    let epoch_min = current_epoch;
    let epoch_max = req.expiry_epoch;

    if epoch_max <= epoch_min {
        return Err((
            axum::http::StatusCode::BAD_REQUEST,
            format!("Expiry epoch ({epoch_max}) must be greater than current epoch ({epoch_min})"),
        ));
    }

    // Build the unsigned subintent
    let subintent_result = transaction_builder::build_unsigned_subintent(
        &req.manifest_text,
        state.network_id,
        epoch_min,
        epoch_max,
    )
    .map_err(|e| {
        tracing::error!("Failed to build subintent: {e}");
        (
            axum::http::StatusCode::BAD_REQUEST,
            format!("Failed to build subintent: {e}"),
        )
    })?;

    // Store the proposal
    let proposal = state
        .proposal_store
        .create(CreateProposal {
            manifest_text: req.manifest_text,
            treasury_account: None,
            epoch_min: epoch_min as i64,
            epoch_max: epoch_max as i64,
            subintent_hash: subintent_result.subintent_hash,
            intent_discriminator: subintent_result.intent_discriminator as i64,
            partial_transaction_bytes: subintent_result.partial_transaction_bytes,
        })
        .await
        .map_err(|e| {
            tracing::error!("Failed to create proposal: {e}");
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to create proposal: {e}"),
            )
        })?;

    Ok(Json(proposal))
}

async fn list_proposals(
    State(state): State<AppState>,
) -> Result<Json<Vec<Proposal>>, (axum::http::StatusCode, String)> {
    state.proposal_store.list().await.map(Json).map_err(|e| {
        tracing::error!("Failed to list proposals: {e}");
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to list proposals: {e}"),
        )
    })
}

async fn get_proposal(
    State(state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<Proposal>, (axum::http::StatusCode, String)> {
    let proposal = state
        .proposal_store
        .get(id)
        .await
        .map_err(|e| {
            tracing::error!("Failed to get proposal: {e}");
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to get proposal: {e}"),
            )
        })?
        .ok_or_else(|| {
            (
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
) -> Result<Json<SignatureStatus>, (axum::http::StatusCode, String)> {
    // Fetch current access rule for validation
    let access_rule = state
        .gateway
        .read_access_rule(&state.multisig_account)
        .await
        .map_err(|e| {
            tracing::error!("Failed to read access rule: {e}");
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to read access rule: {e}"),
            )
        })?;

    state
        .signature_collector
        .add_signature(
            id,
            &req.signed_partial_transaction_hex,
            &access_rule,
            &state.proposal_store,
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
            {
                axum::http::StatusCode::BAD_REQUEST
            } else {
                axum::http::StatusCode::INTERNAL_SERVER_ERROR
            };
            (status, msg)
        })
}

async fn get_signature_status(
    State(state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<SignatureStatus>, (axum::http::StatusCode, String)> {
    let access_rule = state
        .gateway
        .read_access_rule(&state.multisig_account)
        .await
        .map_err(|e| {
            tracing::error!("Failed to read access rule: {e}");
            (
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
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to get signature status: {e}"),
            )
        })
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
    let multisig_account =
        std::env::var("MULTISIG_ACCOUNT_ADDRESS").expect("MULTISIG_ACCOUNT_ADDRESS must be set");
    let frontend_origin =
        std::env::var("FRONTEND_ORIGIN").unwrap_or_else(|_| "http://localhost:3000".into());
    let network_id: u8 = std::env::var("NETWORK_ID")
        .unwrap_or_else(|_| "2".into())
        .parse()
        .expect("NETWORK_ID must be a valid u8");

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
        multisig_account,
        network_id,
    };

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
        .route("/account/access-rule", get(access_rule))
        .route("/proposals", post(create_proposal).get(list_proposals))
        .route("/proposals/{id}", get(get_proposal))
        .route("/proposals/{id}/sign", post(sign_proposal))
        .route("/proposals/{id}/signatures", get(get_signature_status))
        .layer(cors)
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}")).await?;
    tracing::info!("Listening on 0.0.0.0:{port}");

    axum::serve(listener, app).await?;

    Ok(())
}
