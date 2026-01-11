use anyhow::{anyhow, Result};
use axum::{
    extract::{Query, State},
    response::Html,
    routing::get,
    Router,
};
use serde::Deserialize;
use std::net::TcpListener;
use tokio::sync::mpsc;

#[derive(Deserialize)]
struct AuthCallback {
    code: String,
    state: String,
}

struct AppState {
    tx: mpsc::Sender<String>,
    expected_state: String,
}

pub async fn run_server(
    listener: TcpListener,
    expected_state: String,
) -> Result<oauth2::AuthorizationCode> {
    let (tx, mut rx) = mpsc::channel(1);

    let state = std::sync::Arc::new(AppState { tx, expected_state });

    let app = Router::new()
        .route("/callback", get(handler))
        .with_state(state);

    let server = axum::serve(tokio::net::TcpListener::from_std(listener)?, app);

    // Run server in background, but we need to stop it once we get the code.
    // For simplicity in this CLI tool, we'll race the server against the receiver.
    // Actually, axum::serve runs forever. We need graceful shutdown.
    // But simpler: just spawn it and wait for RX.

    tokio::spawn(async move {
        if let Err(e) = server.await {
            eprintln!("Server error: {}", e);
        }
    });

    // Wait for the code
    let code_str = rx
        .recv()
        .await
        .ok_or_else(|| anyhow!("Failed to receive auth code"))?;

    Ok(oauth2::AuthorizationCode::new(code_str))
}

async fn handler(
    Query(params): Query<AuthCallback>,
    State(state): State<std::sync::Arc<AppState>>,
) -> Html<&'static str> {
    if params.state != state.expected_state {
        return Html("<h1>Error: Invalid State</h1><p>CSRF check failed.</p>");
    }

    // Send code back to main thread
    let _ = state.tx.send(params.code).await;

    Html("<h1>Login Successful!</h1><p>You can close this window and return to the terminal.</p><script>window.close()</script>")
}
