# Chat server

In order for notifications to work, you need to set the VAPID keys
and the server URL in the `.env` file.
If the VAPID keys are not set, the server will close, and suggest running `npx web-push generate-vapid-keys`.