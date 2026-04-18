ALTER TABLE repositories
    ADD CONSTRAINT repositories_id_connection_id_unique UNIQUE (id, connection_id);

CREATE TABLE review_agent_runs (
    id TEXT PRIMARY KEY,
    organization_id TEXT NOT NULL REFERENCES organizations(id),
    webhook_id TEXT NOT NULL,
    delivery_attempt_id TEXT NOT NULL,
    connection_id TEXT NOT NULL REFERENCES connections(id),
    repository_id TEXT NOT NULL REFERENCES repositories(id),
    review_id TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'queued',
    created_at TIMESTAMPTZ NOT NULL,
    CONSTRAINT review_agent_runs_status_check CHECK (status IN ('queued', 'claimed', 'completed', 'failed')),
    CONSTRAINT review_agent_runs_repository_visibility_fk FOREIGN KEY (organization_id, repository_id)
        REFERENCES repository_permission_bindings(organization_id, repository_id),
    CONSTRAINT review_agent_runs_repository_connection_fk FOREIGN KEY (repository_id, connection_id)
        REFERENCES repositories(id, connection_id)
);
