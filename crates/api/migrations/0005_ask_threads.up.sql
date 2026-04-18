CREATE TABLE ask_threads (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES sessions(id),
    user_id TEXT NOT NULL REFERENCES local_accounts(id),
    title TEXT NOT NULL,
    repo_scope TEXT[] NOT NULL,
    visibility TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    CONSTRAINT ask_threads_visibility_check CHECK (visibility IN ('private', 'shared')),
    UNIQUE (user_id, session_id)
);
