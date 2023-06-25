use k256::SecretKey;
use rand::{thread_rng, SeedableRng};
use reqwest::StatusCode;
use wasm_bindgen::{JsCast, UnwrapThrowExt};
use web_sys::HtmlInputElement;
use yew::prelude::*;
use yew_hooks::prelude::*;
#[function_component]
fn App() -> Html {
    let username_stored = use_local_storage::<String>("username".to_string());
    let privkey = use_local_storage::<String>("private_key".to_string());
    let username_field = use_state(|| String::new());
    let loc = &use_location();

    let register_action = {
        let privkey = privkey.clone();
        let username_field = username_field.clone();
        let username_stored = username_stored.clone();
        let loc = loc.clone();
        use_async(async move {
            // Make request to server to publish my username and public key.
            let privkey = SecretKey::from_jwk_str(&*privkey.as_ref().expect("jwk key not stored?"))
                .expect("invalid stored jwk key");
            let pubkey = privkey.public_key();
            let username = (*username_field).clone();

            let client = reqwest::Client::builder()
                .build()
                .expect("Failed to build client");
            let result = client
                .post(format!("{}/register/{username}", loc.origin))
                .body(pubkey.to_jwk_string())
                .send()
                .await;
            match result {
                Err(why) => return Err(format!("Error sending registration request: {why}")),
                Ok(res) => {
                    if res.status() == StatusCode::CREATED {
                        username_stored.set(username);
                        return Ok(());
                    } else {
                        let why = res.text().await.unwrap_or("server returned non-text data".to_string());
                        return Err(format!("Error registering: {why}"));
                    }
                }
            }
        })
    };

    if (*username_stored).is_none() {
        // If there is no username set, present the user with a username choice UI.
        // When submitting that username choice, generate new keypair.
        let username_cb = {
            let username = username_field.clone();
            Callback::from(move |event: InputEvent| {
                let event: Event = event.dyn_into().unwrap_throw();
                let event_target = event.target().unwrap_throw();
                let target: HtmlInputElement = event_target.dyn_into().unwrap_throw();
                let username_value = target.value();
                username.set(username_value);
            })
        };

        let register_cb = {
            let privkey = privkey.clone();
            let register_action = register_action.clone();
            Callback::from(move |_e| {
                let mut rng = rand::rngs::StdRng::from_rng(thread_rng()).unwrap();
                let privkey_val = k256::SecretKey::random(&mut rng);
                privkey.set((*(privkey_val.to_jwk_string())).to_string());
                register_action.run();
            })
        };

        return html! {
            <div>
                <h1>{"Choose a username. This will be permanently saved in this browser"}</h1>
                <input type="text" value={(*username_field).clone()} oninput={username_cb} />
                <button onclick={register_cb} disabled={register_action.loading}>{"Register!"}</button>
                {
                    if let Some(error) = &register_action.error {
                        html! { <p style="text-color: red;">{error}</p> }
                    } else {
                        html! {}
                    }
                }
            </div>
        };
    }

    // If here, registration is complete and we have a username to use.

    html!(<ChatWindow />)
}

#[function_component]
fn ChatWindow() -> Html {
    let loc = &use_location();
    let path = format!(
        "ws{}://{}/websocket",
        if loc.protocol == "https" { "s" } else { "" },
        loc.host,
    );
    let options = UseWebSocketOptions {
        onopen: None,
        onmessage: None,
        onmessage_bytes: None,
        onerror: None,
        onclose: None,
        reconnect_limit: None,
        reconnect_interval: None,
        manual: None,
        protocols: None,
    };
    let ws_conn = use_websocket_with_options(path, options);

    html!(<p>{"Chat!"}</p>)
}

fn main() {
    yew::Renderer::<App>::new().render();
}
