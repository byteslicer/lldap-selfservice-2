mod auth;
mod config;
mod db;
mod error;
mod ldap;
mod lldap_client;
mod routes;
mod state;
mod templates;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use axum::Router;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

use crate::config::Config;
use crate::ldap::LdapAuth;
use crate::lldap_client::LldapClient;
use crate::state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse()?))
        .init();

    let config_path = std::env::var("CONFIG_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/etc/lldap-selfservice/config.toml"));

    let config = Arc::new(Config::load(&config_path)?);
    let session_secret = config.session_secret()?;
    let service_password = config.service_password()?;

    let pool = crate::db::connect(&config).await?;
    crate::db::migrate(&pool).await?;

    let lldap = Arc::new(LldapClient::new(&config, service_password));
    lldap.warm_group_cache().await?;

    let state = AppState {
        config: config.clone(),
        pool: pool.clone(),
        ldap: Arc::new(LdapAuth::new(&config)),
        lldap,
        session_secret: Arc::new(session_secret),
    };

    // Periodic session cleanup
    let cleanup_pool = pool.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(3600));
        loop {
            interval.tick().await;
            let _ = crate::db::cleanup_expired(&cleanup_pool).await;
        }
    });

    let static_dir = std::env::var("STATIC_DIR")
        .unwrap_or_else(|_| "./static".to_string());

    let app = Router::new()
        .nest_service("/static", ServeDir::new(static_dir))
        .merge(routes::router(state))
        .layer(TraceLayer::new_for_http());

    let listen: SocketAddr = config.server.listen.parse()?;
    tracing::info!("listening on {listen}");
    let listener = tokio::net::TcpListener::bind(listen).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
