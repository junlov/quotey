//! Web server for portal, dashboard, and PWA
//!
//! Provides HTTP endpoints for:
//! - Customer-facing quote portal (view, approve, reject, comment)
//! - PWA support (manifest, service worker)
//! - Static file serving for web assets

use axum::serve;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tracing::{error, info, warn};

use quotey_db::DbPool;

use crate::portal;

/// Web server handle for graceful shutdown
#[allow(dead_code)]
pub struct WebServerHandle {
    shutdown_tx: tokio::sync::oneshot::Sender<()>,
}

impl WebServerHandle {
    /// Signal the web server to shut down gracefully
    #[allow(dead_code)]
    pub fn shutdown(self) {
        let _ = self.shutdown_tx.send(());
    }
}

/// Spawn the web server with portal routes
///
/// Returns a handle that can be used to trigger graceful shutdown
pub async fn spawn(
    bind_address: &str,
    port: u16,
    db_pool: DbPool,
) -> anyhow::Result<WebServerHandle> {
    let router = portal::router(db_pool);

    let addr: SocketAddr = format!("{}:{}", bind_address, port).parse()?;

    let listener = TcpListener::bind(&addr).await?;
    info!(
        event_name = "system.web_server.bound",
        bind_address = %bind_address,
        port = %port,
        "Web server bound to address"
    );

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();

    tokio::spawn(async move {
        info!(event_name = "system.web_server.starting", "Starting web server with portal routes");

        let server = serve(listener, router);
        let server = server.with_graceful_shutdown(async {
            let _ = shutdown_rx.await;
            info!(
                event_name = "system.web_server.shutdown_signal_received",
                "Web server received shutdown signal"
            );
        });

        if let Err(e) = server.await {
            error!(
                error = %e,
                event_name = "system.web_server.error",
                "Web server encountered an error"
            );
        } else {
            info!(event_name = "system.web_server.stopped", "Web server stopped gracefully");
        }
    });

    info!(
        event_name = "system.web_server.started",
        bind_address = %bind_address,
        port = %port,
        "Web server started successfully"
    );

    Ok(WebServerHandle { shutdown_tx })
}

/// Check if the web server port is available
#[allow(dead_code)]
pub async fn check_port_available(bind_address: &str, port: u16) -> bool {
    match TcpListener::bind(format!("{}:{}", bind_address, port)).await {
        Ok(_) => true,
        Err(e) => {
            warn!(
                error = %e,
                bind_address = %bind_address,
                port = %port,
                "Web server port is not available"
            );
            false
        }
    }
}
