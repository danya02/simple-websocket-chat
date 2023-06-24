# simple-websocket-chat
Barebones chat app based on WebSockets, designed for quick deployment

The server accepts WebSocket connections, and forwards every text message received to every other currently connected client, prefixed by the sending client's name.
The only exception is the first message sent over the socket: this is the client's username.

The server will be running over HTTP; **it is very important** to put it behind a TLS-terminating proxy.