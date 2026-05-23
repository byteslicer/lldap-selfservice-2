use askama::Template;
use chrono::{DateTime, Utc};

use crate::db::InviteRow;
use crate::lldap_client::UserRow;

mod filters {
    pub use super::{format_datetime, invite_status_class, invite_status_label};
}

#[derive(Clone)]
pub struct FlashMessage {
    pub kind: String,
    pub text: String,
    pub invite_url: Option<String>,
}

#[derive(Template)]
#[template(path = "login.html")]
pub struct AdminLoginTemplate {
    pub error: Option<String>,
    pub public_base_url: String,
}

#[derive(Template)]
#[template(path = "dashboard.html")]
pub struct AdminDashboardTemplate {
    pub uid: String,
    pub nav_active: &'static str,
    pub can_reset_pwd: bool,
    pub csrf_token: String,
    pub invites: Vec<InviteRow>,
    pub flash: Option<FlashMessage>,
    pub public_base_url: String,
}

#[derive(Template)]
#[template(path = "users.html")]
pub struct AdminUsersTemplate {
    pub uid: String,
    pub nav_active: &'static str,
    pub can_reset_pwd: bool,
    pub csrf_token: String,
    pub users: Vec<UserRow>,
    pub search: String,
    pub flash: Option<FlashMessage>,
    pub public_base_url: String,
}

#[derive(Template)]
#[template(path = "invite_form.html")]
pub struct InviteFormTemplate {
    pub token: String,
    pub error: Option<String>,
    pub uid: String,
    pub email: String,
}

#[derive(Template)]
#[template(path = "invite_success.html")]
pub struct InviteSuccessTemplate {
    pub uid: String,
}

// Helpers for Askama templates
pub fn format_datetime(dt: &DateTime<Utc>) -> askama::Result<String> {
    Ok(dt.format("%Y-%m-%d %H:%M UTC").to_string())
}

pub fn invite_status_label(inv: &InviteRow) -> askama::Result<String> {
    if inv.used_at.is_some() {
        return Ok("Used".to_string());
    }
    if let Some(exp) = inv.expires_at {
        if exp < Utc::now() {
            return Ok("Expired".to_string());
        }
    }
    Ok("Active".to_string())
}

pub fn invite_status_class(inv: &InviteRow) -> askama::Result<String> {
    if inv.used_at.is_some() {
        return Ok("status-used".to_string());
    }
    if let Some(exp) = inv.expires_at {
        if exp < Utc::now() {
            return Ok("status-expired".to_string());
        }
    }
    Ok("status-active".to_string())
}
