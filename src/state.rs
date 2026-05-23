use std::sync::Arc;

use sqlx::PgPool;

use crate::config::Config;
use crate::ldap::LdapAuth;
use crate::lldap_client::LldapClient;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub pool: PgPool,
    pub ldap: Arc<LdapAuth>,
    pub lldap: Arc<LldapClient>,
    pub session_secret: Arc<Vec<u8>>,
}

impl AppState {
    pub fn invite_ttl_days(&self) -> u64 {
        self.config.server.invite_ttl_days
    }

    pub fn public_base_url(&self) -> &str {
        self.config.server.public_base_url.trim_end_matches('/')
    }
}
