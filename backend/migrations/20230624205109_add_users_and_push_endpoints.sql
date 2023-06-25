CREATE TABLE user (
    name TEXT NOT NULL,
    public_key TEXT NOT NULL
);

CREATE TABLE subscription (
    endpoint TEXT NOT NULL,
    p256dh TEXT NOT NULL,
    auth TEXT NOT NULL
);