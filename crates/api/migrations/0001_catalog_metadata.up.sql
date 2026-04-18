CREATE TABLE connections (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    kind TEXT NOT NULL,
    CONSTRAINT connections_kind_check CHECK (kind IN ('github', 'gitlab', 'local'))
);

CREATE TABLE repositories (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    default_branch TEXT NOT NULL,
    connection_id TEXT NOT NULL REFERENCES connections(id),
    sync_state TEXT NOT NULL,
    CONSTRAINT repositories_sync_state_check CHECK (sync_state IN ('pending', 'ready', 'error'))
);
