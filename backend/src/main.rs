#![feature(async_closure)]
use std::{env, error::Error, path::PathBuf};

use axum::{
    extract::{
        ws::{Message, WebSocket},
        WebSocketUpgrade, Path, State,
    },
    routing::{get, post},
    Router, response::Response, http::{StatusCode},
};
use base64::Engine;
use k256::PublicKey;
use notification::get_notification_router;
use sqlx::{query, SqlitePool};
use tower_http::services::ServeDir;
use web_push::{VapidSignatureBuilder, WebPushClient};

mod notification;

fn say_wrong_keys() {
    println!("VAPID_PRIVATE_KEY is not set or invalid!");
    println!("To fix this, generate a new one with: `npx web-push generate-vapid-keys` and set it in .env");
}

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    dotenvy::from_filename("./backend/.env").expect("Error while loading .env file");
    tracing_subscriber::fmt::init();

    let pool = SqlitePool::connect(&env::var("DATABASE_URL").expect("no DATABASE_URL in .env file?")).await?;
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

    let appstate = AppState { pool };

    // Prepare items to send web push
    let vapid_private_key = vapid_private_key.trim();
    let client = WebPushClient::new()?;
    let signer =
        VapidSignatureBuilder::from_base64_no_sub(vapid_private_key, web_push::URL_SAFE_NO_PAD);
    if signer.is_err() {
        say_wrong_keys();
        signer?;
        return Ok(());
    }
    let signer = signer.unwrap();

    let get_pubkey = {
        let key = signer.get_public_key();
        async move || base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(key)
    };

    let server_url = env::var("SERVER_URL").expect("SERVER_URL should be set in .env file");

    let app = Router::<AppState>::new()
        .route("/ws", get(handle_websocket_connection))
        .route("/vapid_public_key", get(get_pubkey))
        .route("/register/:username", post(register_username))
        .nest("/notification", get_notification_router(client, signer, server_url))
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
        ).with_state(appstate);

    axum::Server::bind(&"0.0.0.0:5000".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .expect("Error while serving?");

    panic!("Exited server code?");
}

async fn register_username(State(appstate): State<AppState>, Path(username): Path<String>, key: String) -> Response<String> {
    async fn inner_register_username( appstate: AppState, username: String, key: String) -> Result<Response<String>, anyhow::Error> {
        let pool = &appstate.pool;
        let existing_user = query!("SELECT * FROM user WHERE name=?", username).fetch_optional(pool).await?;
        if existing_user.is_none(){
            // Try parsing the key to make sure it can be loaded
            let parsed_key = PublicKey::from_jwk_str(&key);
            if parsed_key.is_err() {
                let resp = Response::builder().status(StatusCode::BAD_REQUEST).body(format!("key could not be parsed: {}", parsed_key.unwrap_err())).unwrap();
                return Ok(resp);
            }

            query!("INSERT INTO user (name, public_key) VALUES (?, ?)", username, key).execute(pool).await?;
            let resp = Response::builder().status(StatusCode::CREATED).body("registered user".to_string()).unwrap();
            return Ok(resp);


        } else {
            let resp = Response::builder().status(StatusCode::CONFLICT).body("a user with this username already exists".to_string()).unwrap();
            return Ok(resp);
        }
    }

    let result = inner_register_username(appstate, username, key).await;
    match result {
        Ok(res) => res,
        Err(why) => {
            let resp = Response::builder().status(StatusCode::INTERNAL_SERVER_ERROR).body(format!("database error: {}", why)).unwrap();
            resp
        },
    }
}


async fn handle_websocket_connection(ws: WebSocketUpgrade) -> axum::response::Response {
    ws.on_upgrade(handle_socket)
}

async fn handle_socket(mut socket: WebSocket) {
    while let Some(msg) = socket.recv().await {
        let msg = if let Ok(msg) = msg {
            msg
        } else {
            // client disconnected
            return;
        };

        match msg {
            Message::Text(data) => {
                println!("{}", data);
            }
            _ => {}
        }
    }
}
