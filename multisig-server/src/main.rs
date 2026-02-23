mod gateway;

use std::sync::Arc;

use axum::{extract::State, http::Method, routing::get, Json, Router};
use sqlx::postgres::PgPoolOptions;
use tower_http::cors::{Any, CorsLayer};
use tracing_subscriber::EnvFilter;

use crate::gateway::GatewayClient;

#[derive(Clone)]
pub struct AppState {
    pub pool: sqlx::PgPool,
    pub gateway: Arc<GatewayClient>,
    pub multisig_account: String,
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

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;

    sqlx::migrate!("./migrations").run(&pool).await?;
    tracing::info!("Database migrations applied");

    let gateway = Arc::new(GatewayClient::new(gateway_url));

    let state = AppState {
        pool,
        gateway,
        multisig_account,
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
        .layer(cors)
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}")).await?;
    tracing::info!("Listening on 0.0.0.0:{port}");

    axum::serve(listener, app).await?;

    Ok(())
}
