CREATE TABLE external_accounts (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES local_accounts(id) ON DELETE CASCADE,
    provider TEXT NOT NULL CHECK (length(trim(provider)) > 0),
    provider_user_id TEXT NOT NULL CHECK (length(trim(provider_user_id)) > 0),
    email TEXT NOT NULL,
    name TEXT NOT NULL,
    linked_at TIMESTAMPTZ NOT NULL,
    last_login_at TIMESTAMPTZ,
    UNIQUE (provider, provider_user_id)
);

CREATE INDEX external_accounts_user_id_idx ON external_accounts(user_id);
