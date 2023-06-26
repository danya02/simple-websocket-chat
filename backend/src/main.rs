#![feature(async_closure)]
use std::{borrow::Cow, env, error::Error, path::PathBuf};

use axum::{
    extract::{
        ws::{close_code, CloseFrame, Message, WebSocket},
        Path, State, WebSocketUpgrade,
    },
    http::StatusCode,
    response::Response,
    routing::{get, post},
    Router,
};
use base64::Engine;
use common::ChatMessage;
use k256::PublicKey;
use notification::get_notification_router;
use rand::{seq::SliceRandom, SeedableRng};
use sqlx::{query, SqlitePool};
use tokio::sync::{broadcast, mpsc};
use tower_http::services::ServeDir;
use web_push::{VapidSignatureBuilder, WebPushClient, PartialVapidSignatureBuilder};

use crate::notification::notification_receiver_loop;

mod message_manager;
mod notification;

fn say_wrong_keys() {
    println!("VAPID_PRIVATE_KEY is not set or invalid!");
    println!("To fix this, generate a new one with: `npx web-push generate-vapid-keys` and set it in .env");
}

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub message_manager_tx: mpsc::Sender<ChatMessage>,
    message_manager_broadcaster: broadcast::Sender<ChatMessage>,
    pub webpush_client: WebPushClient,
    pub webpush_signer: PartialVapidSignatureBuilder,
    pub webpush_server_url: String,

}

impl AppState {
    pub fn get_receiver(&self) -> broadcast::Receiver<ChatMessage> {
        self.message_manager_broadcaster.subscribe()
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    dotenvy::from_filename("./backend/.env").expect("Error while loading .env file");
    tracing_subscriber::fmt::init();

    let pool =
        SqlitePool::connect(&env::var("DATABASE_URL").expect("no DATABASE_URL in .env file?"))
            .await?;
    sqlx::migrate!().run(&pool).await?;

    let maybe_vapid_private_key = env::var("VAPID_PRIVATE_KEY");
    let vapid_private_key;
    if maybe_vapid_private_key.is_err() {
        say_wrong_keys();
        return Ok(());
    } else {
        vapid_private_key = maybe_vapid_private_key.unwrap();
        if vapid_private_key.trim().is_empty() {
            say_wrong_keys();
            return Ok(());
        }
    }

    let (message_manager_tx, message_manager_rx) = mpsc::channel(100);
    let (message_broadcaster_tx, message_broadcaster_rx) = broadcast::channel(100);

    tokio::spawn(message_manager::manage_messages(
        message_manager_rx,
        message_broadcaster_tx.clone(),
    ));


    let vapid_private_key = vapid_private_key.trim();
    let client = WebPushClient::new()?;
    let signer =
        VapidSignatureBuilder::from_base64_no_sub(vapid_private_key, web_push::URL_SAFE_NO_PAD);
    if signer.is_err() {
        say_wrong_keys();
        signer?;
        unreachable!();
    }
    let signer = signer.unwrap();

    let get_pubkey = {
        let key = signer.get_public_key();
        async move || base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(key)
    };

    let server_url = env::var("SERVER_URL").expect("SERVER_URL should be set in .env file");


    tokio::spawn(notification_receiver_loop(pool.clone(), signer.clone(), client.clone(), message_broadcaster_rx));


    let appstate = AppState {
        pool,
        message_manager_tx,
        message_manager_broadcaster: message_broadcaster_tx,
        webpush_client: client,
        webpush_signer: signer,
        webpush_server_url: server_url,
    };

    let app = Router::<AppState>::new()
        .route("/ws", get(handle_websocket_connection))
        .route("/vapid_public_key", get(get_pubkey))
        .route("/register/:username", post(register_username))
        .route("/pubkey/:username", get(get_pubkey_by_username))
        .nest(
            "/notification",
            get_notification_router(),
        )
        .nest_service(
            "/",
            ServeDir::new(
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .parent()
                    .unwrap()
                    .join("frontend")
                    .join("dist"),
            )
            .append_index_html_on_directories(true),
        )
        .with_state(appstate);

    axum::Server::bind(&"0.0.0.0:5000".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .expect("Error while serving?");

    panic!("Exited server code?");
}

async fn get_pubkey_by_username(
    State(appstate): State<AppState>,
    Path(username): Path<String>,
) -> Response<String> {
    let pool = &appstate.pool;
    let maybe_existing_user = query!("SELECT * FROM user WHERE name=?", username)
        .fetch_optional(pool)
        .await;
    match maybe_existing_user {
        Err(why) => Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(format!("database error: {why}"))
            .unwrap(),
        Ok(None) => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body("no user found with this username".to_string())
            .unwrap(),
        Ok(Some(data)) => Response::builder()
            .status(StatusCode::OK)
            .body(data.public_key)
            .unwrap(),
    }
}

async fn register_username(
    State(appstate): State<AppState>,
    Path(username): Path<String>,
    key: String,
) -> Response<String> {
    async fn inner_register_username(
        appstate: AppState,
        username: String,
        key: String,
    ) -> Result<Response<String>, anyhow::Error> {
        let pool = &appstate.pool;
        let existing_user = query!("SELECT * FROM user WHERE name=?", username)
            .fetch_optional(pool)
            .await?;
        if existing_user.is_none() {
            // Try parsing the key to make sure it can be loaded
            let parsed_key = PublicKey::from_jwk_str(&key);
            if parsed_key.is_err() {
                let resp = Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .body(format!(
                        "key could not be parsed: {}",
                        parsed_key.unwrap_err()
                    ))
                    .unwrap();
                return Ok(resp);
            }

            query!(
                "INSERT INTO user (name, public_key) VALUES (?, ?)",
                username,
                key
            )
            .execute(pool)
            .await?;
            let resp = Response::builder()
                .status(StatusCode::CREATED)
                .body("registered user".to_string())
                .unwrap();
            return Ok(resp);
        } else {
            let resp = Response::builder()
                .status(StatusCode::CONFLICT)
                .body("a user with this username already exists".to_string())
                .unwrap();
            return Ok(resp);
        }
    }

    let result = inner_register_username(appstate, username, key).await;
    match result {
        Ok(res) => res,
        Err(why) => {
            let resp = Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(format!("database error: {}", why))
                .unwrap();
            resp
        }
    }
}

async fn handle_websocket_connection(
    State(appstate): State<AppState>,
    ws: WebSocketUpgrade,
) -> axum::response::Response {
    let sender = appstate.message_manager_tx.clone();
    let receiver = appstate.get_receiver();
    ws.on_upgrade(|ws| handle_socket(ws, sender, receiver))
}

async fn handle_socket(
    mut socket: WebSocket,
    message_sender: mpsc::Sender<ChatMessage>,
    mut message_receiver: broadcast::Receiver<ChatMessage>,
) {
    // Generate a username to use for simple messages

    let mut rng = rand::rngs::StdRng::from_entropy();
    let mut name = String::from("Anonymous User ");
    let letters = [
        '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'a', 'b', 'c', 'd', 'e', 'f',
    ];
    for _ in 0..4 {
        let letter = letters.choose(&mut rng).unwrap();
        name.push(*letter);
    }

    loop {
        tokio::select! {
            maybe_client_msg = socket.recv() => {
                if let Some(msg) = maybe_client_msg {
                    let msg = if let Ok(msg) = msg {
                        msg
                    } else {
                        // client disconnected
                        message_sender.send(ChatMessage::SystemMessage { content: format!("{name} disconnected from chat") }).await.unwrap();
                        return;
                    };

                    match msg {
                        Message::Text(data) => {
                            // Try to parse the message as a ChatMessage struct.
                            // If we fail, send this message as an anonymous message with no signature.

                            let maybe_parsed_msg: Result<ChatMessage, _> = serde_json::from_str(&data);
                            let process_incoming_msg = async move |data: &str, maybe_parsed_msg: Result<ChatMessage, serde_json::Error>, message_sender: &mpsc::Sender<ChatMessage>, socket: &mut WebSocket, name: &mut String| -> anyhow::Result<()> {
                                match maybe_parsed_msg {
                                    Ok(msg) => {
                                        match msg {
                                            ChatMessage::TextMessage { .. } => message_sender.send(msg).await?,
                                            ChatMessage::SystemMessage { .. } => socket.send(
                                                Message::Text(
                                                    serde_json::to_string(&ChatMessage::SystemMessage { content: format!("Cannot send system messages") }).unwrap()
                                                )
                                            ).await?,
                                            ChatMessage::ConnectionUsername { username } => {
                                                name.clear();
                                                name.extend(username.chars());
                                                message_sender.send(ChatMessage::SystemMessage { content: format!("{username} connected to chat") }).await?;
                                            }
                                        }

                                    },
                                    Err(_) => message_sender.send(ChatMessage::TextMessage { username: name.to_string(), content: data.to_string(), signature: None }).await?,
                                };
                                Ok(())
                            };

                            match process_incoming_msg(&data, maybe_parsed_msg, &message_sender, &mut socket, &mut name).await {
                                Ok(_) => {},
                                Err(_) => {eprintln!("Error while sending message to message manager (are we shutting down?)")},
                            }
                        }
                        _ => {}
                    }
                } else {
                    // client disconnected
                    message_sender.send(ChatMessage::SystemMessage { content: format!("{name} disconnected from chat") }).await.unwrap();
                    return;
                }
            }

            maybe_server_msg = message_receiver.recv() => {
                match maybe_server_msg {
                    Err(_) => {
                        #[allow(unused_must_use)]
                        {
                        socket.send(Message::Close(Some(CloseFrame{ code: close_code::ABNORMAL, reason: Cow::from("Error while retreiving other members' messages (maybe server going down?)") }))).await;
                        socket.close();
                        message_sender.send(ChatMessage::SystemMessage { content: format!("{name} disconnected from chat") }).await.unwrap();
                        }
                        return;
                    },
                    Ok(msg) => {
                        if socket.send(Message::Text(serde_json::to_string(&msg).unwrap())).await.is_err() {
                            // Probably client disconnected?
                            message_sender.send(ChatMessage::SystemMessage { content: format!("{name} disconnected from chat") }).await.unwrap();
                            return;
                        }
                    },
                }
            }
        };
    }
}
