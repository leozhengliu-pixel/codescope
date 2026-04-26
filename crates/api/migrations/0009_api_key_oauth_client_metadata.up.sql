CREATE TABLE api_keys (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES local_accounts(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    secret_hash TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    revoked_at TIMESTAMPTZ,
    repo_scope TEXT[] NOT NULL DEFAULT ARRAY[]::TEXT[]
);

CREATE INDEX api_keys_user_id_idx ON api_keys(user_id, created_at, id);

CREATE TABLE oauth_clients (
    id TEXT PRIMARY KEY,
    organization_id TEXT NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    client_id TEXT NOT NULL UNIQUE,
    client_secret_hash TEXT NOT NULL,
    redirect_uris TEXT[] NOT NULL DEFAULT ARRAY[]::TEXT[],
    created_by_user_id TEXT NOT NULL REFERENCES local_accounts(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL,
    revoked_at TIMESTAMPTZ
);

CREATE INDEX oauth_clients_organization_id_idx
    ON oauth_clients(organization_id, created_at, id);
