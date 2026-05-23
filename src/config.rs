use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub ldap: LdapConfig,
    pub groups: GroupsConfig,
    pub lldap: LldapConfig,
    pub server: ServerConfig,
    pub database: DatabaseConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LdapConfig {
    pub uri: String,
    pub base_dn: String,
    pub people_dn: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GroupsConfig {
    pub invite_admins: String,
    pub password_reset: String,
    pub default_on_signup: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LldapConfig {
    pub http_url: String,
    pub set_password_bin: String,
    pub service_username: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    pub listen: String,
    pub public_base_url: String,
    pub invite_ttl_days: u64,
    #[serde(default = "default_cookie_secure")]
    pub cookie_secure: bool,
}

fn default_cookie_secure() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
}

impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        let contents = fs::read_to_string(path)
            .with_context(|| format!("reading config from {}", path.display()))?;
        let mut config: Config = toml::from_str(&contents).context("parsing config TOML")?;
        config.apply_env_overrides();
        Ok(config)
    }

    fn apply_env_overrides(&mut self) {
        if let Ok(url) = std::env::var("DATABASE_URL") {
            self.database.url = url;
        }
        if let Ok(url) = std::env::var("LLDAP_HTTP_URL") {
            self.lldap.http_url = url;
        }
        if let Ok(bin) = std::env::var("LLDAP_SET_PASSWORD_BIN") {
            self.lldap.set_password_bin = bin;
        }
        if let Ok(user) = std::env::var("LLDAP_SERVICE_USERNAME") {
            self.lldap.service_username = user;
        }
        if let Ok(listen) = std::env::var("LISTEN") {
            self.server.listen = listen;
        }
        if let Ok(base) = std::env::var("PUBLIC_BASE_URL") {
            self.server.public_base_url = base;
        }
    }

    pub fn session_secret(&self) -> Result<Vec<u8>> {
        let path = std::env::var("SESSION_SECRET_FILE")
            .unwrap_or_else(|_| "/run/secrets/lldap_selfservice_session".to_string());
        let secret = fs::read_to_string(&path)
            .with_context(|| format!("reading session secret from {path}"))?;
        let secret = secret.trim();
        anyhow::ensure!(
            secret.len() >= 32,
            "session secret must be at least 32 characters"
        );
        Ok(secret.as_bytes().to_vec())
    }

    pub fn service_password(&self) -> Result<String> {
        let path = std::env::var("LLDAP_SERVICE_PASSWORD_FILE")
            .unwrap_or_else(|_| "/run/secrets/lldap_selfservice_service_pass".to_string());
        let pass = fs::read_to_string(&path)
            .with_context(|| format!("reading LLDAP service password from {path}"))?;
        Ok(pass.trim().to_string())
    }
}

// Minimal TOML parsing without extra dep — use serde with toml via adding toml dep
// Actually I used toml::from_str but didn't add toml to Cargo.toml. Let me add it.
