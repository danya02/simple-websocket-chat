use common::ChatMessage;
use wasm_bindgen::JsCast;
use wasm_bindgen::UnwrapThrowExt;
use web_sys::HtmlInputElement;
use yew::prelude::*;
use yew_hooks::prelude::*;

#[function_component]
pub fn ChatWindow() -> Html {
    let loc = &use_location();
    let path = format!(
        "ws{}://{}/ws",
        if loc.protocol == "https:" { "s" } else { "" },
        loc.host,
    );

    let chat_history: UseListHandle<ChatMessage> = use_list(vec![]);
    let did_send_username = use_state_eq(|| false);

    let username = use_local_storage::<String>("username".to_string());
    let privkey = use_local_storage::<String>("private_key".to_string());


    let options = UseWebSocketOptions {
        onopen: None,
        onmessage: Some({
            let chat_history = chat_history.clone();
            Box::new(move |message| {
                let message_parsed = serde_json::from_str(&message);
                match message_parsed {
                    Ok(msg) => chat_history.push(msg),
                    Err(why) => chat_history.push(ChatMessage::SystemMessage { content: format!("Server sent an unexpected message: {why}") }),
                }
            })
        }),
        onmessage_bytes: None,
        onerror: None,
        onclose: None,
        reconnect_limit: Some(u32::MAX), // Never give up!
        reconnect_interval: None,
        manual: None,
        protocols: None,
    };
    let ws_conn = use_websocket_with_options(path, options);

    let text_value = use_state(|| String::new());
    let oninput_cb = {
        let text_value = text_value.clone();
        Callback::from(move |e: InputEvent| {
            let event: Event = e.dyn_into().unwrap_throw();
            let event_target = event.target().unwrap_throw();
            let target: HtmlInputElement = event_target.dyn_into().unwrap_throw();
            let val = target.value();
            text_value.set(val);
        })
    };

    let send_cb = {
        let ws_conn = ws_conn.clone();
        let text_value = text_value.clone();
        Callback::from(move |e: SubmitEvent| {
            ws_conn.send((*text_value).clone());
            text_value.set(String::new());
            e.prevent_default();
        })
    };

    match *ws_conn.ready_state {
        UseWebSocketReadyState::Connecting => {
            did_send_username.set(false);
            html!(<h2>{"Connecting to chat websocket..."}</h2>)
        },
        UseWebSocketReadyState::Closing => {
            html!(<h2>{"Websocket is closing (this should never happen?!)..."}</h2>)
        }
        UseWebSocketReadyState::Closed => {
            did_send_username.set(false);
            html!(<h2>{"Websocket is closed, reconnecting..."}</h2>)
        }
        UseWebSocketReadyState::Open => {
            if !(*did_send_username){
                let username = username.clone();
                let username = (*username).clone().expect_throw("no username while in chat window code?!");
                ws_conn.send(serde_json::to_string(&ChatMessage::ConnectionUsername { username }).unwrap());
                did_send_username.set(true);
            }
            html!(
                <div>
                    {
                        for chat_history.current().iter().map(|message| {
                            html! {
                                <MessageDisplay message={message.clone()} />
                            }
                        })
                    }
                    <form onsubmit={send_cb}>
                        <input type="text" oninput={oninput_cb} value={(*text_value).clone()} />
                        <input type="submit" value="Send!" />
                    </form>
                </div>
            )
        }
    }
}

#[derive(Properties, PartialEq, Clone)]
struct MessageDisplayProps {
    pub message: ChatMessage,
}

#[function_component]
fn MessageDisplay(props: &MessageDisplayProps) -> Html {
    match &props.message {
        ChatMessage::TextMessage { username, content, signature } => html!{
            <p><span style="text-color: blue;">{&username}</span>{":"}<span>{&content}</span></p>
        },
        ChatMessage::SystemMessage { content } => html!{
            <p style="text-color: red;">{&content}</p>
        },
        ChatMessage::ConnectionUsername { .. } => html!{<h1>{format!("{:?} (should never see this)", &props.message)}</h1>}
    }
}