use askama_axum::IntoResponse;
use axum::extract::{Form, Path, State};
use axum::response::IntoResponse as AxumIntoResponse;
use axum::routing::get;
use axum::Router;
use chrono::Utc;
use serde::Deserialize;

use crate::auth::hash_invite_token;
use crate::db;
use crate::error::{AppError, AppResult};
use crate::ldap::{validate_email, validate_password, validate_uid};
use crate::state::AppState;
use crate::templates::{InviteFormTemplate, InviteSuccessTemplate};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/invite/:token", get(invite_form).post(invite_submit))
}

async fn load_valid_invite(
    state: &AppState,
    token: &str,
) -> AppResult<(db::InviteRow, Vec<u8>)> {
    let hash = hash_invite_token(token.as_bytes());
    let invite = db::find_invite_by_token_hash(&state.pool, &hash)
        .await?
        .ok_or_else(|| AppError::msg("Invite not found"))?;

    if invite.used_at.is_some() {
        return Err(AppError::msg("Invite already used"));
    }
    if let Some(exp) = invite.expires_at {
        if exp < Utc::now() {
            return Err(AppError::msg("Invite has expired"));
        }
    }
    Ok((invite, hash))
}

async fn invite_form(
    State(state): State<AppState>,
    Path(token): Path<String>,
) -> AppResult<impl AxumIntoResponse> {
    match load_valid_invite(&state, &token).await {
        Ok(_) => Ok(InviteFormTemplate {
            token,
            error: None,
            uid: String::new(),
            email: String::new(),
        }
        .into_response()),
        Err(e) => {
            let msg = e.to_string();
            Ok((
                axum::http::StatusCode::BAD_REQUEST,
                InviteFormTemplate {
                    token: String::new(),
                    error: Some(msg),
                    uid: String::new(),
                    email: String::new(),
                },
            )
                .into_response())
        }
    }
}

#[derive(Deserialize)]
pub struct InviteSignupForm {
    uid: String,
    email: String,
    password: String,
    password_confirm: String,
}

async fn invite_submit(
    State(state): State<AppState>,
    Path(token): Path<String>,
    Form(form): Form<InviteSignupForm>,
) -> AppResult<impl AxumIntoResponse> {
    let (invite, _hash) = load_valid_invite(&state, &token).await?;

    let uid = form.uid.trim().to_lowercase();
    let email = form.email.trim().to_lowercase();

    validate_uid(&uid)?;
    validate_email(&email)?;
    validate_password(&form.password)?;

    if form.password != form.password_confirm {
        return Err(AppError::msg("Passwords do not match"));
    }

    if state.lldap.user_exists(&uid).await.map_err(AppError::from)? {
        return Err(AppError::msg("Username already taken"));
    }

    // Check email uniqueness via user list
    let users = state
        .lldap
        .list_users(None)
        .await
        .map_err(AppError::from)?;
    if users.iter().any(|u| u.email.eq_ignore_ascii_case(&email)) {
        return Err(AppError::msg("Email already in use"));
    }

    state
        .lldap
        .create_user(&uid, &email, &uid)
        .await
        .map_err(AppError::from)?;

    state
        .lldap
        .add_user_to_default_groups(&uid)
        .await
        .map_err(AppError::from)?;

    state
        .lldap
        .set_password(&uid, &form.password)
        .await
        .map_err(AppError::from)?;

    let marked = db::mark_invite_used(&state.pool, invite.id, &uid).await?;
    if !marked {
        return Err(AppError::msg("Invite already used"));
    }

    Ok(InviteSuccessTemplate { uid: uid.clone() }.into_response())
}
