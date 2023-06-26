// This code bridges to Rust code in order to provide easier access to the Web Push API.
// from https://github.com/mdn/serviceworker-cookbook/blob/master/push-subscription-management/index.js

window.sw_pushmanager = null;
window.pushmanager_subscription = null;

navigator.serviceWorker.ready
    .then(function(registration) {
        console.log("Service worker registered");
        window.sw_pushmanager = registration.pushManager;
        return registration.pushManager
    }).then(function(manager) {
      manager.getSubscription().then(
        function(subscription){window.pushmanager_subscription = subscription}
      );
    });
// The above should be completed by the time that the other methods get called

function get_subscription() {
    // Return whether there is a current active subscription.
    return !(window.pushmanager_subscription === null);
}

// This function is needed because Chrome doesn't accept a base64 encoded string
// as value for applicationServerKey in pushManager.subscribe yet
// https://bugs.chromium.org/p/chromium/issues/detail?id=802280
// https://github.com/mdn/serviceworker-cookbook/blob/master/tools.js
function urlBase64ToUint8Array(base64String) {
    var padding = '='.repeat((4 - base64String.length % 4) % 4);
    var base64 = (base64String + padding)
      .replace(/\-/g, '+')
      .replace(/_/g, '/');
   
    var rawData = window.atob(base64);
    var outputArray = new Uint8Array(rawData.length);
   
    for (var i = 0; i < rawData.length; ++i) {
      outputArray[i] = rawData.charCodeAt(i);
    }
    return outputArray;
  }


function resubscribe() {
    // Add a subscription, and if there is an old subscription, remove it.
    navigator.serviceWorker.ready
    .then(async function(registration) {
        // Get the server's public key
        const response = await fetch('/vapid_public_key');
        const vapidPublicKey = await response.text();
        // Chrome doesn't accept the base64-encoded (string) vapidPublicKey yet
        // urlBase64ToUint8Array() is defined in /tools.js
        const convertedVapidKey = urlBase64ToUint8Array(vapidPublicKey);
        // Subscribe the user

        let do_registration_flow = function() {
          registration.pushManager.subscribe({
              userVisibleOnly: true,
              applicationServerKey: convertedVapidKey
            }).then(function(subscription) {
              console.log('Subscribed', subscription.endpoint);
              window.pushmanager_subscription = subscription;
              return fetch('/notification/register', {
                method: 'post',
                headers: {
                  'Content-type': 'application/json'
                },
                body: JSON.stringify(
                  subscription.toJSON()
                )
              });
          });
        };

        // If there is an existing registration, we need to cancel it first.
        // Otherwise, just call the registration flow.
        registration.pushManager.getSubscription().then(function(current_subscription){
          if(current_subscription === null) {
            do_registration_flow();
          } else {
            current_subscription.unsubscribe().then(function(){
              window.pushmanager_subscription = null;
              return fetch('/notification/unregister', {
                method: 'post',
                headers: {
                  'Content-type': 'application/json'
                },
                body: JSON.stringify(
                  current_subscription.toJSON()
                )
              });
            }).then(do_registration_flow);
          }
        });
      }
    )
}

