use anyhow::{anyhow, Result};
use axum::{
    extract::{Query, State},
    response::Html,
    routing::get,
    Router,
};
use serde::Deserialize;
use std::net::TcpListener;
use tokio::sync::{mpsc, oneshot};

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
    let (ready_tx, ready_rx) = oneshot::channel();

    let state = std::sync::Arc::new(AppState { tx, expected_state });

    let app = Router::new()
        .route("/callback", get(handler.clone()))
        .route("/oauth2callback", get(handler))
        .with_state(state);

    // Convert std listener to tokio listener
    listener.set_nonblocking(true)?;
    let tokio_listener = tokio::net::TcpListener::from_std(listener)?;

    // Spawn server
    tokio::spawn(async move {
        // Signal that we're ready to accept connections
        let _ = ready_tx.send(());

        if let Err(e) = axum::serve(tokio_listener, app).await {
            eprintln!("Server error: {}", e);
        }
    });

    // Wait for server to be ready
    let _ = ready_rx.await;

    // Small delay to ensure server is fully listening
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

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
