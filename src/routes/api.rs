use askama_axum::IntoResponse;
use axum::extract::{Form, Path, State};
use axum::response::{IntoResponse as AxumIntoResponse, Redirect};
use axum::routing::{get, post};
use axum::Json;
use axum_extra::extract::cookie::CookieJar;
use chrono::{Duration, Utc};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::auth::{
    generate_invite_token, load_session, verify_csrf,
};
use crate::db;
use crate::error::{AppError, AppResult};
use crate::ldap::validate_password;
use crate::state::AppState;
use crate::templates::{AdminDashboardTemplate, FlashMessage};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/invites", get(list_invites).post(create_invite))
        .route(
            "/api/users/:uid/reset-password",
            post(reset_password),
        )
}

use axum::Router;

async fn require_invite_session(
    state: &AppState,
    jar: &CookieJar,
) -> AppResult<db::SessionRow> {
    let session = load_session(state, jar)
        .await?
        .ok_or_else(|| AppError::msg("Unauthorized"))?;
    if !session.can_invite {
        return Err(AppError::msg("Forbidden"));
    }
    Ok(session)
}

async fn list_invites(
    State(state): State<AppState>,
    jar: CookieJar,
) -> AppResult<Json<serde_json::Value>> {
    let _session = require_invite_session(&state, &jar).await?;
    let invites = db::list_invites(&state.pool).await?;
    Ok(Json(json!({ "invites": invites })))
}

#[derive(Deserialize)]
pub struct CreateInviteForm {
    label: Option<String>,
    csrf_token: String,
}

async fn create_invite(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<CreateInviteForm>,
) -> AppResult<impl AxumIntoResponse> {
    let session = require_invite_session(&state, &jar).await?;
    verify_csrf(&session, &form.csrf_token)?;

    let (raw_token, token_hash) = generate_invite_token();
    let invite_id = Uuid::new_v4();
    let expires_at = if state.invite_ttl_days() > 0 {
        Some(Utc::now() + Duration::days(state.invite_ttl_days() as i64))
    } else {
        None
    };

    db::create_invite(
        &state.pool,
        invite_id,
        &token_hash,
        &session.uid,
        expires_at,
        form.label.as_deref(),
    )
    .await?;

    let invite_url = format!("{}/invite/{}", state.public_base_url(), raw_token);

    // HTMX or form post: return HTML fragment with link
    let invites = db::list_invites(&state.pool).await?;
    Ok(AdminDashboardTemplate {
        uid: session.uid,
        nav_active: "invites",
        can_reset_pwd: session.can_reset_pwd,
        csrf_token: session.csrf_token,
        invites,
        flash: Some(FlashMessage {
            kind: "success".into(),
            text: "Invite link created. Copy it now — it will not be shown again.".into(),
            invite_url: Some(invite_url),
        }),
        public_base_url: state.public_base_url().to_string(),
    }
    .into_response())
}

#[derive(Deserialize)]
pub struct ResetPasswordForm {
    password: String,
    password_confirm: String,
    csrf_token: String,
}

async fn reset_password(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(uid): Path<String>,
    Form(form): Form<ResetPasswordForm>,
) -> AppResult<impl AxumIntoResponse> {
    let session = load_session(&state, &jar)
        .await?
        .ok_or_else(|| AppError::msg("Unauthorized"))?;
    if !session.can_reset_pwd {
        return Err(AppError::msg(
            "Forbidden: password reset group membership required",
        ));
    }
    verify_csrf(&session, &form.csrf_token)?;

    if form.password != form.password_confirm {
        return Err(AppError::msg("Passwords do not match"));
    }
    validate_password(&form.password)?;

    let uid = uid.trim().to_lowercase();
    if !state.lldap.user_exists(&uid).await.map_err(AppError::from)? {
        return Err(AppError::msg("User not found"));
    }

    if state
        .lldap
        .user_is_lldap_admin(&uid)
        .await
        .map_err(AppError::from)?
    {
        return Err(AppError::msg(
            "Cannot reset password for LLDAP admin accounts",
        ));
    }

    state
        .lldap
        .set_password(&uid, &form.password)
        .await
        .map_err(AppError::from)?;

    Ok(Redirect::to(&format!(
        "/admin/users?q={}&reset=ok",
        urlencoding::encode(&uid)
    )))
}
