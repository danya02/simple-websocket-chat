use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen::{prelude::Closure};
use wasm_bindgen::{UnwrapThrowExt};
use web_sys::{Notification, NotificationPermission, NotificationOptions};
use yew::prelude::*;

#[wasm_bindgen]
extern "C" {
    fn get_subscription() -> bool;
    fn resubscribe();
}

#[function_component]
pub fn WebPushSetup() -> Html {
    let refresh_pulse = use_state(|| ());
    let closure = use_state(move || Closure::new(move |_| refresh_pulse.set(())));
    let request_permission_cb = use_callback(move |_,_| {
        let result = Notification::request_permission().unwrap_throw();
        #[allow(unused_must_use)]
        {
            // We set up this promise, but do not care about its result.
            // When it resolves, the refresh pulse will happen, and the new notification permission will be fetched during the render.
            result.then(&*closure);
        }
    }, ());

    let permission = Notification::permission();

    let send_notification_cb = use_callback(|_,_|{
        let mut options = NotificationOptions::new();
        options.body("This is an example notification!");
        let _notification = Notification::new_with_options("Test", &options);
        // The notification only needs to be created in order to be shown
    }, ());

    let resubscribe_cb = use_callback(|_,_| {
        #[allow(unused_unsafe)]  // this unsafe is actually needed
        unsafe { resubscribe(); }
    }, ());

    html! {
        <div>
            <p>{"Current notification permission state:"}{format!("{permission:?}")}</p>
            <button onclick={request_permission_cb}>{"Ask for permission to send you notifications"}</button>
            {
                if permission == NotificationPermission::Granted {
                    #[allow(unused_unsafe)]  // this unsafe is actually needed
                    let state = unsafe {get_subscription()};
                    html!(
                        <>
                            <button onclick={send_notification_cb}>{"Send an example notification"}</button>
                            <p>{"Web Push subscription appears to be active: "}{state}</p>
                            <button onclick={resubscribe_cb}>{"Refresh Web Push subscription (will send a notification if successful)"}</button>
                        </>
                    )
                } else {html!()}
            }
        </div>
    }
}
