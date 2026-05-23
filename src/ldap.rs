use crate::config::{Config, GroupsConfig, LdapConfig};
use crate::error::{AppError, AppResult};
use ldap3::{LdapConn, Scope};

#[derive(Clone)]
pub struct LdapAuth {
    config: LdapConfig,
    groups: GroupsConfig,
}

impl LdapAuth {
    pub fn new(config: &Config) -> Self {
        Self {
            config: config.ldap.clone(),
            groups: config.groups.clone(),
        }
    }

    /// Bind with user credentials and return whether they may invite / reset passwords.
    pub async fn authenticate(&self, username: &str, password: &str) -> AppResult<(bool, bool)> {
        let this = self.clone();
        let username = username.to_string();
        let password = password.to_string();
        tokio::task::spawn_blocking(move || this.authenticate_blocking(&username, &password))
            .await
            .map_err(|e| AppError::msg(format!("LDAP task failed: {e}")))?
    }

    fn authenticate_blocking(&self, username: &str, password: &str) -> AppResult<(bool, bool)> {
        let bind_dn = format!("uid={},{}", username, self.config.people_dn);
        let mut ldap = LdapConn::new(&self.config.uri)
            .map_err(|e| AppError::msg(format!("LDAP connection failed: {e}")))?;
        ldap.simple_bind(&bind_dn, password)
            .map_err(|e| AppError::msg(format!("LDAP bind failed: {e}")))?
            .success()
            .map_err(|_| AppError::msg("Invalid credentials"))?;

        let can_invite = self.user_in_group(&mut ldap, username, &self.groups.invite_admins)?;
        let can_reset = self.user_in_group(&mut ldap, username, &self.groups.password_reset)?;

        if !can_invite && !can_reset {
            return Err(AppError::msg(
                "Forbidden: you are not a member of any self-service admin group",
            ));
        }

        let _ = ldap.unbind();
        Ok((can_invite, can_reset))
    }

    fn user_in_group(
        &self,
        ldap: &mut LdapConn,
        username: &str,
        group_dn: &str,
    ) -> AppResult<bool> {
        // groupOfUniqueNames: uniqueMember points to user DN
        let user_dn = format!("uid={},{}", username, self.config.people_dn);
        let filter = format!("(&(objectClass=groupOfUniqueNames)(uniqueMember={user_dn}))");
        let rs = ldap
            .search(group_dn, Scope::Base, &filter, vec!["cn"])
            .map_err(|e| AppError::msg(format!("LDAP group search failed: {e}")))?
            .success()
            .map_err(|e| AppError::msg(format!("LDAP group search error: {e}")))?;

        Ok(!rs.0.is_empty())
    }

    /// Check if uid exists via search under people OU.
    pub fn uid_exists(&self, username: &str) -> AppResult<bool> {
        let mut ldap = self.service_bind()?;
        // uid is validated before calls; safe in filter
        let filter = format!("(&(objectClass=person)(uid={username}))");
        let rs = ldap
            .search(
                &self.config.people_dn,
                Scope::Subtree,
                &filter,
                vec!["uid"],
            )
            .map_err(|e| AppError::msg(format!("LDAP search failed: {e}")))?
            .success()
            .map_err(|e| AppError::msg(format!("LDAP search error: {e}")))?;
        let _ = ldap.unbind();
        Ok(!rs.0.is_empty())
    }

    fn service_bind(&self) -> AppResult<LdapConn> {
        // For uid_exists during invite we use admin bind from env — optional path via GraphQL instead
        let bind_dn = std::env::var("LDAP_BIND_DN").unwrap_or_else(|_| {
            format!(
                "uid={},{}",
                std::env::var("LLDAP_SERVICE_USERNAME").unwrap_or_else(|_| "admin".into()),
                self.config.people_dn
            )
        });
        let password = std::env::var("LDAP_BIND_PASSWORD")
            .or_else(|_| {
                let path = std::env::var("LLDAP_SERVICE_PASSWORD_FILE")
                    .unwrap_or_else(|_| "/run/secrets/lldap_selfservice_service_pass".into());
                std::fs::read_to_string(path).map(|s| s.trim().to_string())
            })
            .map_err(|e| AppError::msg(format!("LDAP bind password unavailable: {e}")))?;

        let mut ldap = LdapConn::new(&self.config.uri)
            .map_err(|e| AppError::msg(format!("LDAP connection failed: {e}")))?;
        ldap.simple_bind(&bind_dn, &password)
            .map_err(|e| AppError::msg(format!("LDAP service bind failed: {e}")))?
            .success()
            .map_err(|e| AppError::msg(format!("LDAP service bind error: {e}")))?;
        Ok(ldap)
    }
}

/// Validate LDAP uid: alphanumeric, underscore, hyphen, dot; 1-32 chars
pub fn validate_uid(uid: &str) -> AppResult<()> {
    if uid.is_empty() || uid.len() > 32 {
        return Err(AppError::msg("Username must be 1–32 characters"));
    }
    if !uid
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
    {
        return Err(AppError::msg(
            "Username may only contain letters, numbers, underscore, hyphen, and dot",
        ));
    }
    if uid.starts_with('.') || uid.ends_with('.') {
        return Err(AppError::msg("Username may not start or end with a dot"));
    }
    Ok(())
}

pub fn validate_password(password: &str) -> AppResult<()> {
    if password.len() < 8 {
        return Err(AppError::msg("Password must be at least 8 characters"));
    }
    Ok(())
}

pub fn validate_email(email: &str) -> AppResult<()> {
    if !email.contains('@') || email.len() > 254 {
        return Err(AppError::msg("Invalid email address"));
    }
    Ok(())
}
