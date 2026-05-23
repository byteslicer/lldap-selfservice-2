pub mod admin;
pub mod api;
pub mod invite;

use axum::response::Redirect;
use axum::routing::get;
use axum::Router;

use crate::state::AppState;

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/", get(index))
        .merge(admin::router())
        .merge(invite::router())
        .merge(api::router())
        .with_state(state)
}

async fn index() -> Redirect {
    Redirect::to("/admin")
}
