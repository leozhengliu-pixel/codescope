CREATE TABLE delivery_attempts (
    id TEXT PRIMARY KEY,
    webhook_id TEXT NOT NULL,
    connection_id TEXT NOT NULL REFERENCES connections(id),
    repository_id TEXT NOT NULL REFERENCES repositories(id),
    event_type TEXT NOT NULL,
    review_id TEXT NOT NULL,
    external_event_id TEXT NOT NULL,
    accepted_at TIMESTAMPTZ NOT NULL,
    CONSTRAINT delivery_attempts_repository_connection_fk FOREIGN KEY (repository_id, connection_id)
        REFERENCES repositories(id, connection_id)
);
