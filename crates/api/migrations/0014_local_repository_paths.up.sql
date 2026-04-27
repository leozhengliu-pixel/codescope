CREATE TABLE local_repository_paths (
    repository_id TEXT PRIMARY KEY REFERENCES repositories(id) ON DELETE CASCADE,
    connection_id TEXT NOT NULL REFERENCES connections(id),
    canonical_path TEXT NOT NULL,
    CONSTRAINT local_repository_paths_connection_path_unique UNIQUE (connection_id, canonical_path)
);
