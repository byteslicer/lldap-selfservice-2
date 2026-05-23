use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use hmac::{Hmac, Mac};
use rand::RngCore;
use sha2::Sha256;
use subtle::ConstantTimeEq;
use uuid::Uuid;

use crate::db::{self, SessionRow};
use crate::error::{AppError, AppResult};
use crate::state::AppState;

type HmacSha256 = Hmac<Sha256>;

pub const SESSION_COOKIE: &str = "lldap_selfservice_session";

pub fn hash_invite_token(token: &[u8]) -> Vec<u8> {
    let mut hasher = sha2::Sha256::new();
    use sha2::Digest;
    hasher.update(token);
    hasher.finalize().to_vec()
}

pub fn generate_invite_token() -> (String, Vec<u8>) {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    let raw = hex::encode(bytes);
    let hash = hash_invite_token(raw.as_bytes());
    (raw, hash)
}

pub fn generate_csrf_token() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}

pub fn sign_session_id(secret: &[u8], session_id: Uuid) -> String {
    let mut mac =
        HmacSha256::new_from_slice(secret).expect("HMAC accepts any key size");
    mac.update(session_id.as_bytes());
    let sig = mac.finalize().into_bytes();
    format!("{}.{}", session_id, hex::encode(sig))
}

pub fn verify_session_cookie(secret: &[u8], value: &str) -> AppResult<Uuid> {
    let (id_str, sig_hex) = value
        .split_once('.')
        .ok_or_else(|| AppError::msg("Invalid session cookie"))?;
    let session_id = Uuid::parse_str(id_str)
        .map_err(|_| AppError::msg("Invalid session cookie"))?;

    let mut mac =
        HmacSha256::new_from_slice(secret).expect("HMAC accepts any key size");
    mac.update(session_id.as_bytes());
    let expected = mac.finalize().into_bytes();

    let sig = hex::decode(sig_hex).map_err(|_| AppError::msg("Invalid session cookie"))?;
    if expected.as_slice().ct_eq(&sig).unwrap_u8() != 1 {
        return Err(AppError::msg("Invalid session cookie"));
    }
    Ok(session_id)
}

pub async fn load_session(state: &AppState, jar: &CookieJar) -> AppResult<Option<SessionRow>> {
    let Some(cookie) = jar.get(SESSION_COOKIE) else {
        return Ok(None);
    };
    let session_id = verify_session_cookie(&state.session_secret, cookie.value())?;
    let session = db::get_session(&state.pool, session_id).await?;
    Ok(session)
}

pub fn set_session_cookie(
    state: &AppState,
    jar: CookieJar,
    session_id: Uuid,
) -> CookieJar {
    let signed = sign_session_id(&state.session_secret, session_id);
    let mut cookie = Cookie::new(SESSION_COOKIE, signed);
    cookie.set_http_only(true);
    cookie.set_path("/");
    cookie.set_same_site(SameSite::Lax);
    if state.config.server.cookie_secure {
        cookie.set_secure(true);
    }
    jar.add(cookie)
}

pub fn clear_session_cookie(jar: CookieJar) -> CookieJar {
    let cookie = Cookie::build((SESSION_COOKIE, ""))
        .path("/")
        .max_age(time::Duration::ZERO)
        .build();
    jar.remove(cookie)
}

pub fn verify_csrf(session: &SessionRow, token: &str) -> AppResult<()> {
    if session.csrf_token.as_bytes().ct_eq(token.as_bytes()).unwrap_u8() != 1 {
        return Err(AppError::msg("Invalid CSRF token"));
    }
    Ok(())
}
