use axum::{extract::State, http::StatusCode, response::Response, routing::post, Json, Router};
use common::ChatMessage;
use serde::{Serialize};
use sqlx::{query, SqlitePool};
use tokio::sync::broadcast;
use web_push::{PartialVapidSignatureBuilder, SubscriptionInfo, WebPushClient, WebPushMessageBuilder};

use crate::AppState;

pub fn get_notification_router(
) -> Router<AppState> {
    Router::new()
        .route("/register", post(add_registration))
        .route("/unregister", post(remove_registration))
}

async fn add_registration(
    State(appstate): State<AppState>,
    Json(data): Json<SubscriptionInfo>,
) -> Response<String> {
    let pool = &appstate.pool;
    let result = query!(
        "INSERT INTO subscription (endpoint, p256dh, auth) VALUES (?,?,?)",
        data.endpoint,
        data.keys.p256dh,
        data.keys.auth
    )
    .execute(pool)
    .await;
    let mut resp = Response::new(String::new());
    if result.is_err() {
        *resp.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
    }

    test_notification(&data.into(), &appstate.webpush_signer, &appstate.webpush_client).await.expect("failed to send notification?");
    return resp;
}

async fn remove_registration(
    State(appstate): State<AppState>,
    Json(data): Json<SubscriptionInfo>,
) -> Response<String> {
    let pool = &appstate.pool;
    let result = query!(
        "DELETE FROM subscription WHERE endpoint=?",
        data.endpoint,  // endpoint is the only important property for a registration
    )
    .execute(pool)
    .await;
    let mut resp = Response::new(String::new());
    if result.is_err() {
        *resp.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
    }
    return resp;
}

#[derive(Serialize)]
struct Notification {
    pub title: String,
    pub body: String,
    /// If true, the browser is instructed to show the notification even if it is currently focused.
    pub always_show: bool,
}

pub async fn test_notification(info: &SubscriptionInfo, signer: &PartialVapidSignatureBuilder, client: &WebPushClient) -> anyhow::Result<()> {
    let signer = signer.clone().add_sub_info(info);
    let mut builder = WebPushMessageBuilder::new(info)?;
    let content = serde_json::to_vec(&Notification{ title: String::from("Test Push notification"), body: String::from("This is what incoming chat messages will look like"), always_show: true}).unwrap();
    builder.set_payload(web_push::ContentEncoding::Aes128Gcm, &content);
    builder.set_vapid_signature(signer.build()?);

    client.send(builder.build()?).await?;

    Ok(())
}

pub async fn notification_receiver_loop(pool: SqlitePool, signer: PartialVapidSignatureBuilder, client: WebPushClient, mut receiver: broadcast::Receiver<ChatMessage>) {
    loop {
        let msg = receiver.recv().await;
        match msg {
            Err(why) => {
                eprintln!("Error receiving message in notifier loop: {why}");
            },
            Ok(msg) => {
                match msg {
                    ChatMessage::TextMessage { username, content, .. } => {
                        // Broadcast this message to all subscribers
                        let subscriptions = sqlx::query!("SELECT * FROM subscription;").fetch_all(&pool).await;
                        match subscriptions {
                            Err(why) => eprintln!("Error fetching subscriptions: {why}"),
                            Ok(subs) => {
                                let subs = subs.iter().map(|sub| (sub.endpoint.clone(), sub.p256dh.clone(), sub.auth.clone())).collect::<Vec<_>>();
                                let send_to_sub = async move |sub: (String, String, String), signer: PartialVapidSignatureBuilder, client: WebPushClient, username: String, content: String| -> anyhow::Result<()> {
                                    let info = SubscriptionInfo::new(sub.0, sub.1, sub.2);
                                    let signer = signer.add_sub_info(&info);
                                    let mut builder = WebPushMessageBuilder::new(&info)?;
                                    let content = serde_json::to_vec(&Notification{ title: username, body: content, always_show: false}).unwrap();
                                    builder.set_payload(web_push::ContentEncoding::Aes128Gcm, &content);
                                    builder.set_vapid_signature(signer.build()?);
                                
                                    client.send(builder.build()?).await?;
                                    Ok(())
                                };

                                for sub in subs {
                                    let send = {
                                        let client = client.clone();
                                        let signer = signer.clone();
                                        let username = username.clone();
                                        let content = content.clone();

                                        async move {
                                            let sub_to_send = sub.clone();
                                            match send_to_sub(sub_to_send, signer, client, username, content).await {
                                                Ok(_) => {},
                                                Err(why) => eprintln!("Error sending to subscription {sub:?}: {why}"),
                                            }
                                        }
                                    };
                                    tokio::spawn(send);
                                }
                            },
                        }
                    },
                    _ => {}
                }
            }
        }
    }
}