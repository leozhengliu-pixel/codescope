CREATE TABLE repository_sync_jobs (
    id TEXT PRIMARY KEY,
    organization_id TEXT NOT NULL REFERENCES organizations(id),
    repository_id TEXT NOT NULL REFERENCES repositories(id),
    connection_id TEXT NOT NULL REFERENCES connections(id),
    status TEXT NOT NULL,
    queued_at TIMESTAMPTZ NOT NULL,
    started_at TIMESTAMPTZ,
    finished_at TIMESTAMPTZ,
    error TEXT,
    CONSTRAINT repository_sync_jobs_status_check CHECK (status IN ('queued', 'running', 'succeeded', 'failed'))
);

CREATE INDEX repository_sync_jobs_claim_idx
    ON repository_sync_jobs (queued_at, id)
    WHERE status = 'queued';

CREATE INDEX repository_sync_jobs_organization_idx
    ON repository_sync_jobs (organization_id, queued_at, id);
