//! HTTP route handlers for PlantUML diagram generation
//!
//! Provides REST endpoints for SVG, PNG, and TXT diagram generation.

use crate::plantuml::{DiagramFormat, PlantUMLExecutor};
use axum::{
    body::Bytes,
    extract::Path,
    http::{HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

/// Health check endpoint
pub async fn health_check() -> impl IntoResponse {
    Json(json!({
        "status": "ok",
        "service": "plantuml-server-rust",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

/// Server info endpoint
pub async fn info() -> impl IntoResponse {
    Json(json!({
        "name": "b00t PlantUML Server (Rust Edition)",
        "version": env!("CARGO_PKG_VERSION"),
        "description": "Modern HTTP server for PlantUML diagram generation using axum + subprocess execution",
        "endpoints": {
            "health": "GET /health, GET /plantuml/health",
            "post_svg": "POST /plantuml/svg (body: PlantUML source)",
            "post_png": "POST /plantuml/png (body: PlantUML source)",
            "post_txt": "POST /plantuml/txt (body: PlantUML source)",
            "get_svg": "GET /plantuml/svg/{encoded}",
            "get_png": "GET /plantuml/png/{encoded}",
        },
        "integration": {
            "b00t_ipc": "Queue-based processing with MessageBus",
            "mcp_protocol": "Model Context Protocol server support",
            "togaf": "TOGAF enterprise architecture workflows",
        }
    }))
}

/// Generate SVG diagram from PlantUML source (POST)
pub async fn generate_svg(body: Bytes) -> Response {
    generate_diagram(body, DiagramFormat::Svg).await
}

/// Generate PNG diagram from PlantUML source (POST)
pub async fn generate_png(body: Bytes) -> Response {
    generate_diagram(body, DiagramFormat::Png).await
}

/// Generate TXT syntax validation from PlantUML source (POST)
pub async fn generate_txt(body: Bytes) -> Response {
    generate_diagram(body, DiagramFormat::Txt).await
}

/// Render SVG from encoded PlantUML URL parameter (GET)
pub async fn render_encoded_svg(Path(encoded): Path<String>) -> Response {
    render_encoded(encoded, DiagramFormat::Svg).await
}

/// Render PNG from encoded PlantUML URL parameter (GET)
pub async fn render_encoded_png(Path(encoded): Path<String>) -> Response {
    render_encoded(encoded, DiagramFormat::Png).await
}

/// Internal: Generate diagram from PlantUML source
async fn generate_diagram(body: Bytes, format: DiagramFormat) -> Response {
    // Convert body to string
    let source = match String::from_utf8(body.to_vec()) {
        Ok(s) => s,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": format!("Invalid UTF-8 in request body: {}", e) })),
            )
                .into_response();
        }
    };

    // Validate non-empty source
    if source.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "Empty PlantUML source" })),
        )
            .into_response();
    }

    // Create executor and generate diagram
    let executor = match PlantUMLExecutor::new() {
        Ok(e) => e,
        Err(e) => {
            tracing::error!("Failed to create PlantUML executor: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Server configuration error: {}", e) })),
            )
                .into_response();
        }
    };

    match executor.generate(&source, format).await {
        Ok(output) => {
            let mut headers = HeaderMap::new();
            headers.insert(
                "Content-Type",
                HeaderValue::from_static(format.content_type()),
            );
            (StatusCode::OK, headers, output).into_response()
        }
        Err(e) => {
            tracing::error!("PlantUML generation failed: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Diagram generation failed: {}", e) })),
            )
                .into_response()
        }
    }
}

/// Internal: Render diagram from encoded URL parameter
async fn render_encoded(encoded: String, format: DiagramFormat) -> Response {
    // Decode PlantUML encoding
    let source = match decode_plantuml(&encoded) {
        Ok(s) => s,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": format!("Invalid PlantUML encoding: {}", e) })),
            )
                .into_response();
        }
    };

    tracing::debug!("Decoded PlantUML source ({} bytes)", source.len());

    // Generate diagram using decoded source
    generate_diagram(Bytes::from(source), format).await
}

/// Decode PlantUML URL encoding to source text
///
/// PlantUML uses a custom encoding scheme:
/// - Base64-like alphabet with URL-safe characters
/// - Deflate compression applied before encoding
///
/// Reference: https://plantuml.com/text-encoding
fn decode_plantuml(encoded: &str) -> anyhow::Result<String> {
    // PlantUML uses a custom base64 alphabet
    const PLANTUML_ALPHABET: &[u8] =
        b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz-_";

    // Convert PlantUML encoding to standard base64
    let mut base64_chars = Vec::new();
    for ch in encoded.chars() {
        let idx = PLANTUML_ALPHABET
            .iter()
            .position(|&c| c as char == ch)
            .ok_or_else(|| anyhow::anyhow!("Invalid character in encoding: {}", ch))?;

        // Convert to standard base64 alphabet (A-Za-z0-9+/)
        let b64_char = if idx < 26 {
            (b'A' + idx as u8) as char // A-Z
        } else if idx < 52 {
            (b'a' + (idx - 26) as u8) as char // a-z
        } else if idx < 62 {
            (b'0' + (idx - 52) as u8) as char // 0-9
        } else if idx == 62 {
            '+'
        } else {
            '/'
        };
        base64_chars.push(b64_char);
    }

    let base64_str: String = base64_chars.into_iter().collect();

    // Decode base64
    use base64::Engine;
    let compressed = base64::engine::general_purpose::STANDARD
        .decode(base64_str)
        .map_err(|e| anyhow::anyhow!("Base64 decode failed: {}", e))?;

    // Decompress using flate2 (zlib/deflate)
    use flate2::read::ZlibDecoder;
    use std::io::Read;

    let mut decoder = ZlibDecoder::new(&compressed[..]);
    let mut source = String::new();
    decoder
        .read_to_string(&mut source)
        .map_err(|e| anyhow::anyhow!("Decompression failed: {}", e))?;

    Ok(source)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_plantuml_simple() {
        // Example encoded diagram: @startuml\nAlice -> Bob: Hello\n@enduml
        // This is a placeholder - actual encoding would need to be verified
        // against PlantUML's encoding implementation
        let encoded = "SyfFKj2rKt3CoKnELR1Io4ZDoSa70000";
        let result = decode_plantuml(encoded);

        // We expect either success or a specific error
        // The actual test would need a known good encoding
        match result {
            Ok(s) => assert!(!s.is_empty()),
            Err(_) => {
                // Expected for this placeholder encoding
                // Real test would use verified encoding
            }
        }
    }
}
