CREATE TABLE sessions (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES local_accounts(id),
    secret_hash TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL
);
