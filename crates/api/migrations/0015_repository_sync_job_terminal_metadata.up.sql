ALTER TABLE repository_sync_jobs
    ADD COLUMN synced_revision TEXT,
    ADD COLUMN synced_branch TEXT,
    ADD COLUMN synced_content_file_count BIGINT;
