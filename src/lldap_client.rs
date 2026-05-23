use std::collections::HashMap;
use std::process::Stdio;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use serde::Deserialize;
use serde_json::json;
use tokio::process::Command;
use tokio::sync::RwLock;

use crate::config::Config;

#[derive(Clone)]
pub struct LldapClient {
    http_url: String,
    set_password_bin: String,
    service_username: String,
    service_password: String,
    default_group_dns: Vec<String>,
    token: Arc<RwLock<CachedToken>>,
    group_cache: Arc<RwLock<HashMap<String, i64>>>,
}

struct CachedToken {
    jwt: String,
    expires_at: Instant,
}

#[derive(Deserialize)]
struct LoginResponse {
    token: String,
}

#[derive(Deserialize)]
struct GraphQlResponse<T> {
    data: Option<T>,
    errors: Option<Vec<GraphQlError>>,
}

#[derive(Deserialize)]
struct GraphQlError {
    message: String,
}

#[derive(Deserialize)]
struct GroupsData {
    groups: Vec<GroupRow>,
}

#[derive(Deserialize)]
struct GroupRow {
    id: i64,
    #[serde(rename = "displayName")]
    display_name: String,
}

#[derive(Deserialize)]
struct UsersData {
    users: Vec<UserRow>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UserRow {
    pub id: String,
    pub email: String,
    #[serde(rename = "displayName")]
    pub display_name: String,
}

#[derive(Deserialize)]
struct CreateUserData {
    createUser: CreateUserId,
}

#[derive(Deserialize)]
struct CreateUserId {
    id: String,
}

#[derive(Deserialize)]
struct UserDetailData {
    user: UserDetail,
}

#[derive(Deserialize)]
struct UserDetail {
    id: String,
    groups: Vec<GroupRow>,
}

impl LldapClient {
    pub fn new(config: &Config, service_password: String) -> Self {
        Self {
            http_url: config.lldap.http_url.trim_end_matches('/').to_string(),
            set_password_bin: config.lldap.set_password_bin.clone(),
            service_username: config.lldap.service_username.clone(),
            service_password,
            default_group_dns: config.groups.default_on_signup.clone(),
            token: Arc::new(RwLock::new(CachedToken {
                jwt: String::new(),
                expires_at: Instant::now(),
            })),
            group_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn warm_group_cache(&self) -> Result<()> {
        let _ = self.jwt().await?;
        let groups = self.fetch_groups().await?;
        let mut cache = self.group_cache.write().await;
        cache.clear();
        for g in groups {
            cache.insert(g.display_name.clone(), g.id);
        }
        Ok(())
    }

    async fn jwt(&self) -> Result<String> {
        {
            let guard = self.token.read().await;
            if !guard.jwt.is_empty() && guard.expires_at > Instant::now() {
                return Ok(guard.jwt.clone());
            }
        }
        let client = reqwest::Client::new();
        let url = format!("{}/auth/simple/login", self.http_url);
        let resp: LoginResponse = client
            .post(&url)
            .json(&json!({
                "username": self.service_username,
                "password": self.service_password,
            }))
            .send()
            .await
            .context("LLDAP login request")?
            .error_for_status()
            .context("LLDAP login failed")?
            .json()
            .await
            .context("LLDAP login JSON")?;

        let mut guard = self.token.write().await;
        guard.jwt = resp.token;
        guard.expires_at = Instant::now() + Duration::from_secs(23 * 3600);
        Ok(guard.jwt.clone())
    }

    async fn graphql<T: for<'de> Deserialize<'de>>(
        &self,
        query: &str,
        variables: serde_json::Value,
    ) -> Result<T> {
        let token = self.jwt().await?;
        let client = reqwest::Client::new();
        let url = format!("{}/api/graphql", self.http_url);
        let body = json!({ "query": query, "variables": variables });
        let resp: GraphQlResponse<T> = client
            .post(&url)
            .header("Authorization", format!("Bearer {token}"))
            .json(&body)
            .send()
            .await
            .context("GraphQL request")?
            .error_for_status()
            .context("GraphQL HTTP error")?
            .json()
            .await
            .context("GraphQL JSON")?;

        if let Some(errors) = resp.errors {
            let msg = errors
                .iter()
                .map(|e| e.message.as_str())
                .collect::<Vec<_>>()
                .join("; ");
            anyhow::bail!("GraphQL errors: {msg}");
        }
        resp.data
            .context("GraphQL response missing data")
    }

    async fn fetch_groups(&self) -> Result<Vec<GroupRow>> {
        const QUERY: &str = r#"query { groups { id displayName } }"#;
        let data: GroupsData = self.graphql(QUERY, json!({})).await?;
        Ok(data.groups)
    }

    async fn group_id_for_dn(&self, group_dn: &str) -> Result<i64> {
        // Extract cn=NAME from DN
        let name = group_dn
            .strip_prefix("cn=")
            .and_then(|s| s.split(',').next())
            .context("invalid group DN")?;

        {
            let cache = self.group_cache.read().await;
            if let Some(&id) = cache.get(name) {
                return Ok(id);
            }
        }

        self.warm_group_cache().await?;
        let cache = self.group_cache.read().await;
        cache
            .get(name)
            .copied()
            .with_context(|| format!("group not found in LLDAP: {name}"))
    }

    pub async fn user_exists(&self, uid: &str) -> Result<bool> {
        const QUERY: &str =
            r#"query($id: String!) { user(userId: $id) { id } }"#;
        let token = self.jwt().await?;
        let client = reqwest::Client::new();
        let url = format!("{}/api/graphql", self.http_url);
        let body = json!({
            "query": QUERY,
            "variables": { "id": uid }
        });
        let resp: GraphQlResponse<serde_json::Value> = client
            .post(&url)
            .header("Authorization", format!("Bearer {token}"))
            .json(&body)
            .send()
            .await?
            .json()
            .await?;

        if let Some(errors) = &resp.errors {
            if errors.iter().any(|e| e.message.contains("not found")) {
                return Ok(false);
            }
            let msg = errors
                .iter()
                .map(|e| e.message.as_str())
                .collect::<Vec<_>>()
                .join("; ");
            anyhow::bail!("GraphQL errors: {msg}");
        }
        Ok(resp.data.is_some())
    }

    pub async fn list_users(&self, filter: Option<&str>) -> Result<Vec<UserRow>> {
        const QUERY: &str = r#"query { users { id email displayName } }"#;
        let data: UsersData = self.graphql(QUERY, json!({})).await?;
        let users = data.users;
        if let Some(q) = filter {
            let q = q.to_lowercase();
            Ok(users
                .into_iter()
                .filter(|u| {
                    u.id.to_lowercase().contains(&q)
                        || u.email.to_lowercase().contains(&q)
                        || u.display_name.to_lowercase().contains(&q)
                })
                .collect())
        } else {
            Ok(users)
        }
    }

    pub async fn create_user(&self, uid: &str, email: &str, display_name: &str) -> Result<()> {
        const MUTATION: &str = r#"
            mutation CreateUser($user: CreateUserInput!) {
                createUser(user: $user) { id }
            }
        "#;
        let _: CreateUserData = self
            .graphql(
                MUTATION,
                json!({
                    "user": {
                        "id": uid,
                        "email": email,
                        "displayName": display_name,
                    }
                }),
            )
            .await?;
        Ok(())
    }

    pub async fn add_user_to_default_groups(&self, uid: &str) -> Result<()> {
        for group_dn in &self.default_group_dns {
            let group_id = self.group_id_for_dn(group_dn).await?;
            self.add_user_to_group(uid, group_id).await?;
        }
        Ok(())
    }

    async fn add_user_to_group(&self, uid: &str, group_id: i64) -> Result<()> {
        const MUTATION: &str = r#"
            mutation($userId: String!, $groupId: Int!) {
                addUserToGroup(userId: $userId, groupId: $groupId) { success }
            }
        "#;
        #[derive(Deserialize)]
        struct R {
            addUserToGroup: Success,
        }
        #[derive(Deserialize)]
        struct Success {
            success: bool,
        }
        let _: R = self
            .graphql(
                MUTATION,
                json!({ "userId": uid, "groupId": group_id }),
            )
            .await?;
        Ok(())
    }

    pub async fn user_is_lldap_admin(&self, uid: &str) -> Result<bool> {
        const QUERY: &str = r#"query($id: String!) { user(userId: $id) { groups { displayName } } }"#;
        let data: UserDetailData = self.graphql(QUERY, json!({ "id": uid })).await?;
        Ok(data
            .user
            .groups
            .iter()
            .any(|g| g.display_name == "lldap_admin"))
    }

    pub async fn set_password(&self, uid: &str, password: &str) -> Result<()> {
        let token = self.jwt().await?;
        let output = Command::new(&self.set_password_bin)
            .args([
                "--url",
                &self.http_url,
                "--jwt-token",
                &token,
                "--user",
                uid,
                "--password",
                password,
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .with_context(|| format!("running {}", self.set_password_bin))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            anyhow::bail!(
                "lldap_set_password failed: status={} stderr={stderr} stdout={stdout}",
                output.status
            );
        }
        Ok(())
    }
}
