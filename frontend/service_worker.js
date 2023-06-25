// From https://github.com/fkohlgrueber/yew-pwa-minimal

var cacheName = 'yew-chat-pwa';
var filesToCache = [
  './',
  './index.html',
  './frontend.js',
  './frontend_bg.wasm',
  './manifest.json',
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