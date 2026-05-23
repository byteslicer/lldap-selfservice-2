use askama_axum::IntoResponse;
use axum::extract::{Form, Query, State};
use axum::response::{IntoResponse as AxumIntoResponse, Redirect};
use axum::routing::{get, post};
use axum::Router;
use axum_extra::extract::cookie::CookieJar;
use serde::Deserialize;
use uuid::Uuid;

use crate::auth::{
    clear_session_cookie, generate_csrf_token, load_session, set_session_cookie,
};
use crate::db;
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::templates::{AdminDashboardTemplate, AdminLoginTemplate, AdminUsersTemplate};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/admin/login", get(login_form).post(login_submit))
        .route("/admin/logout", post(logout))
        .route("/admin", get(dashboard))
        .route("/admin/users", get(users_page))
}

#[derive(Deserialize)]
pub struct LoginForm {
    username: String,
    password: String,
}

#[derive(Deserialize)]
pub struct UsersQuery {
    q: Option<String>,
}

async fn login_form(State(state): State<AppState>) -> AdminLoginTemplate {
    AdminLoginTemplate {
        error: None,
        public_base_url: state.public_base_url().to_string(),
    }
}

async fn login_submit(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<LoginForm>,
) -> AppResult<impl AxumIntoResponse> {
    let username = form.username.trim().to_lowercase();
    if username.is_empty() {
        return Ok(AdminLoginTemplate {
            error: Some("Username is required".into()),
            public_base_url: state.public_base_url().to_string(),
        }
        .into_response());
    }

    let (can_invite, can_reset) = state
        .ldap
        .authenticate(&username, &form.password)
        .await
        .map_err(|_| AppError::msg("Invalid credentials or insufficient permissions"))?;

    let session_id = Uuid::new_v4();
    let csrf = generate_csrf_token();
    db::create_session(
        &state.pool,
        session_id,
        &username,
        can_invite,
        can_reset,
        &csrf,
        24,
    )
    .await?;

    let jar = set_session_cookie(&state, jar, session_id);
    let dest = if can_invite { "/admin" } else { "/admin/users" };
    Ok((jar, Redirect::to(dest)).into_response())
}

async fn logout(
    State(state): State<AppState>,
    jar: CookieJar,
) -> AppResult<impl AxumIntoResponse> {
    if let Ok(Some(session)) = load_session(&state, &jar).await {
        let _ = db::delete_session(&state.pool, session.id).await;
    }
    let jar = clear_session_cookie(jar);
    Ok((jar, Redirect::to("/admin/login")))
}

async fn dashboard(
    State(state): State<AppState>,
    jar: CookieJar,
) -> AppResult<impl AxumIntoResponse> {
    let session = match load_session(&state, &jar).await? {
        Some(s) if s.can_invite => s,
        Some(s) if s.can_reset_pwd => {
            return Ok(Redirect::to("/admin/users").into_response());
        }
        Some(_) => return Ok(Redirect::to("/admin/login").into_response()),
        None => return Ok(Redirect::to("/admin/login").into_response()),
    };

    let invites = db::list_invites(&state.pool).await?;
    Ok(AdminDashboardTemplate {
        uid: session.uid.clone(),
        can_reset_pwd: session.can_reset_pwd,
        csrf_token: session.csrf_token.clone(),
        invites,
        flash: None,
        public_base_url: state.public_base_url().to_string(),
    }
    .into_response())
}

async fn users_page(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(query): Query<UsersQuery>,
) -> AppResult<impl AxumIntoResponse> {
    let session = load_session(&state, &jar)
        .await?
        .ok_or_else(|| AppError::msg("Unauthorized"))?;
    if !session.can_invite && !session.can_reset_pwd {
        return Err(AppError::msg("Forbidden"));
    }
    let users = state
        .lldap
        .list_users(query.q.as_deref())
        .await
        .map_err(AppError::from)?;

    Ok(AdminUsersTemplate {
        uid: session.uid,
        can_reset_pwd: session.can_reset_pwd,
        csrf_token: session.csrf_token,
        users,
        search: query.q.unwrap_or_default(),
        flash: None,
        public_base_url: state.public_base_url().to_string(),
    }
    .into_response())
}
