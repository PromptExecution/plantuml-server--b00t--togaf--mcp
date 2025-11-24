//! PlantUML Server - Rust Edition
//!
//! Modern HTTP server for PlantUML diagram generation using axum + subprocess execution.
//! Designed for b00t TOGAF workflows with queue integration and MCP protocol support.

mod plantuml;
mod routes;

use anyhow::Result;
use axum::{
    Router,
    routing::{get, post},
};
use std::net::SocketAddr;
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "plantuml_server_rust=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("🥾 b00t PlantUML Server starting...");

    // Build application router
    let app = Router::new()
        // Health check endpoint
        .route("/health", get(routes::health_check))
        .route("/plantuml/health", get(routes::health_check))
        // PlantUML generation endpoints
        .route("/plantuml/svg", post(routes::generate_svg))
        .route("/plantuml/png", post(routes::generate_png))
        .route("/plantuml/txt", post(routes::generate_txt))
        // Encoded diagram URLs (PlantUML standard)
        .route("/plantuml/svg/:encoded", get(routes::render_encoded_svg))
        .route("/plantuml/png/:encoded", get(routes::render_encoded_png))
        // Info endpoint
        .route("/", get(routes::info))
        // CORS layer for web access
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        // Request tracing
        .layer(TraceLayer::new_for_http());

    // Server configuration
    let port = std::env::var("PORT")
        .unwrap_or_else(|_| "8080".to_string())
        .parse::<u16>()?;

    let addr = SocketAddr::from(([0, 0, 0, 0], port));

    tracing::info!("🚀 Server listening on http://{}", addr);
    tracing::info!("📊 Health check: http://{}/health", addr);
    tracing::info!("🎨 Generate SVG: POST http://{}/plantuml/svg", addr);

    // Start server
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
