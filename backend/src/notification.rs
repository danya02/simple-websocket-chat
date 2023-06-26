use axum::{extract::State, http::StatusCode, response::Response, routing::post, Json, Router};
use serde::{Deserialize, Serialize};
use sqlx::query;
use web_push::{PartialVapidSignatureBuilder, SubscriptionInfo, SubscriptionKeys, WebPushClient};

use crate::AppState;

pub fn get_notification_router(
    client: WebPushClient,
    signer: PartialVapidSignatureBuilder,
    server_url: String,
) -> Router<AppState> {
    Router::new()
        .route("/register", post(add_registration))
        .route("/unregister", post(remove_registration))
}

async fn add_registration(
    State(appstate): State<AppState>,
    Json(data): Json<RegistrationData>,
) -> Response<String> {
    let pool = &appstate.pool;
    let result = query!(
        "INSERT INTO subscription (endpoint, p256dh, auth) VALUES (?,?,?)",
        data.endpoint,
        data.p256dh,
        data.auth
    )
    .execute(pool)
    .await;
    let mut resp = Response::new(String::new());
    if result.is_err() {
        *resp.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
    }
    return resp;
}

async fn remove_registration(
    State(appstate): State<AppState>,
    Json(data): Json<RegistrationData>,
) -> Response<String> {
    let pool = &appstate.pool;
    let result = query!(
        "DELETE FROM subscription WHERE endpoint=? AND p256dh=? AND auth=?",
        data.endpoint,
        data.p256dh,
        data.auth
    )
    .execute(pool)
    .await;
    let mut resp = Response::new(String::new());
    if result.is_err() {
        *resp.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
    }
    return resp;
}

#[derive(Serialize, Deserialize)]
struct RegistrationData {
    pub endpoint: String,
    pub p256dh: String,
    pub auth: String,
}

impl Into<SubscriptionInfo> for RegistrationData {
    fn into(self) -> SubscriptionInfo {
        let keys = SubscriptionKeys {
            p256dh: self.p256dh,
            auth: self.auth,
        };
        SubscriptionInfo {
            endpoint: self.endpoint,
            keys,
        }
    }
}
