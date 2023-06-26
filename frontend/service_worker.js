// From https://github.com/fkohlgrueber/yew-pwa-minimal

var cacheName = 'yew-chat-pwa';
var filesToCache = [
  './',
  './index.html',
  './frontend.js',
  './frontend_bg.wasm',
  './manifest.json',
  './web_push_bridge.js',
  './icon/chat-right-dots.png',
  './icon/chat-right-dots.svg',
];


/* Start the service worker and cache all of the app's content */
self.addEventListener('install', function(e) {
  e.waitUntil(
    caches.open(cacheName).then(function(cache) {
      return cache.addAll(filesToCache);
    })
  );
});

/* Serve cached content when offline */
self.addEventListener('fetch', function(e) {
  e.respondWith(
    caches.match(e.request).then(function(response) {
      return response || fetch(e.request);
    })
  );
});

// from https://web.dev/push-notifications-common-notification-patterns/
function isClientFocused() {
  return clients
    .matchAll({
      type: 'window',
      includeUncontrolled: true,
    })
    .then((windowClients) => {
      let clientIsFocused = false;

      for (let i = 0; i < windowClients.length; i++) {
        const windowClient = windowClients[i];
        if (windowClient.focused) {
          clientIsFocused = true;
          break;
        }
      }

      return clientIsFocused;
    });
}

// From https://github.com/mdn/serviceworker-cookbook/blob/master/push-subscription-management/service-worker.js
self.addEventListener('push', function(event) {
  event.waitUntil(

    isClientFocused().then((clientIsFocused) => {
      if (clientIsFocused && !event.data.json().always_show) {
        console.log("Don't need to show a notification.");
        return;
      }
    
      // Client isn't focused, we need to show a notification.
        return self.registration.showNotification(event.data.json().title, {
          body: event.data.json().body
        }).then(function(){console.log("Received push ",event.data.json())})
      })
  );
});

self.addEventListener('pushsubscriptionchange', function(event) {
  console.log('Subscription expired');
  event.waitUntil(
    self.registration.pushManager.subscribe(event.oldSubscription.options)
    .then(function(subscription) {
      console.log("Requesting to unsubscribe from old subscription and subscribe to new subscription");
      fetch('/notification/unregister', {
        method: 'post',
        headers: {
          'Content-type': 'application/json'
        },
        body: JSON.stringify({
            endpoint: subscription.endpoint,
            auth: subscription.getKey('auth'),
            p256dh: subscription.getKey('p256dh'),

        })
      }).then(function() {
        console.log('Subscribed after expiration', subscription.endpoint);
        return fetch('/notification/register', {
          method: 'post',
          headers: {
            'Content-type': 'application/json'
          },
          body: JSON.stringify({
            endpoint: subscription.endpoint,
            auth: subscription.getKey('auth'),
            p256dh: subscription.getKey('p256dh'),
          })
        });}
        ).catch(function() {
          self.registration.showNotification('Chat Push Notification error', {
            body: 'When resubscribing to notifications, encountered an unexpected error. Notifications may fail now. Open the app to fix this.'
          })
        });
      }
    )
  )
});
