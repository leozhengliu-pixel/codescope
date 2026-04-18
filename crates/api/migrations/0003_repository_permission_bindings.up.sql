CREATE TABLE repository_permission_bindings (
    organization_id TEXT NOT NULL REFERENCES organizations(id),
    repository_id TEXT NOT NULL REFERENCES repositories(id),
    synced_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (organization_id, repository_id)
);
