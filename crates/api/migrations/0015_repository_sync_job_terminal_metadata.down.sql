ALTER TABLE repository_sync_jobs
    DROP COLUMN synced_content_file_count,
    DROP COLUMN synced_branch,
    DROP COLUMN synced_revision;
