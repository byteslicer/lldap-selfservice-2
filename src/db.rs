use chrono::{DateTime, Duration, Utc};
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use uuid::Uuid;

use crate::config::Config;
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct InviteRow {
    pub id: Uuid,
    pub created_by_uid: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub used_at: Option<DateTime<Utc>>,
    pub lldap_user_id: Option<String>,
    pub label: Option<String>,
}

#[derive(Debug, sqlx::FromRow)]
pub struct SessionRow {
    pub id: Uuid,
    pub uid: String,
    pub can_invite: bool,
    pub can_reset_pwd: bool,
    pub expires_at: DateTime<Utc>,
    pub csrf_token: String,
}

pub async fn connect(config: &Config) -> AppResult<PgPool> {
    PgPoolOptions::new()
        .max_connections(5)
        .connect(&config.database.url)
        .await
        .map_err(AppError::from)
}

pub async fn migrate(pool: &PgPool) -> AppResult<()> {
    sqlx::migrate!("./migrations")
        .run(pool)
        .await
        .map_err(|e| AppError::msg(format!("migration failed: {e}")))?;
    Ok(())
}

pub async fn create_invite(
    pool: &PgPool,
    id: Uuid,
    token_hash: &[u8],
    created_by_uid: &str,
    expires_at: Option<DateTime<Utc>>,
    label: Option<&str>,
) -> AppResult<()> {
    sqlx::query(
        r#"
        INSERT INTO invites (id, token_hash, created_by_uid, expires_at, label)
        VALUES ($1, $2, $3, $4, $5)
        "#,
    )
    .bind(id)
    .bind(token_hash)
    .bind(created_by_uid)
    .bind(expires_at)
    .bind(label)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn list_invites(pool: &PgPool) -> AppResult<Vec<InviteRow>> {
    let rows = sqlx::query_as::<_, InviteRow>(
        r#"
        SELECT id, created_by_uid, created_at, expires_at, used_at, lldap_user_id, label
        FROM invites
        ORDER BY created_at DESC
        LIMIT 100
        "#,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn find_invite_by_token_hash(
    pool: &PgPool,
    token_hash: &[u8],
) -> AppResult<Option<InviteRow>> {
    let row = sqlx::query_as::<_, InviteRow>(
        r#"
        SELECT id, created_by_uid, created_at, expires_at, used_at, lldap_user_id, label
        FROM invites
        WHERE token_hash = $1
        "#,
    )
    .bind(token_hash)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn mark_invite_used(
    pool: &PgPool,
    id: Uuid,
    lldap_user_id: &str,
) -> AppResult<bool> {
    let result = sqlx::query(
        r#"
        UPDATE invites
        SET used_at = NOW(), lldap_user_id = $2
        WHERE id = $1 AND used_at IS NULL
        "#,
    )
    .bind(id)
    .bind(lldap_user_id)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() == 1)
}

pub async fn create_session(
    pool: &PgPool,
    id: Uuid,
    uid: &str,
    can_invite: bool,
    can_reset_pwd: bool,
    csrf_token: &str,
    ttl_hours: i64,
) -> AppResult<()> {
    let expires_at = Utc::now() + Duration::hours(ttl_hours);
    sqlx::query(
        r#"
        INSERT INTO sessions (id, uid, can_invite, can_reset_pwd, expires_at, csrf_token)
        VALUES ($1, $2, $3, $4, $5, $6)
        "#,
    )
    .bind(id)
    .bind(uid)
    .bind(can_invite)
    .bind(can_reset_pwd)
    .bind(expires_at)
    .bind(csrf_token)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_session(pool: &PgPool, id: Uuid) -> AppResult<Option<SessionRow>> {
    let row = sqlx::query_as::<_, SessionRow>(
        r#"
        SELECT id, uid, can_invite, can_reset_pwd, expires_at, csrf_token
        FROM sessions
        WHERE id = $1 AND expires_at > NOW()
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn delete_session(pool: &PgPool, id: Uuid) -> AppResult<()> {
    sqlx::query("DELETE FROM sessions WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn cleanup_expired(pool: &PgPool) -> AppResult<()> {
    sqlx::query("DELETE FROM sessions WHERE expires_at <= NOW()")
        .execute(pool)
        .await?;
    Ok(())
}
