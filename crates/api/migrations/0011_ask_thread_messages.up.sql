ALTER TABLE ask_threads
    ADD COLUMN messages JSONB NOT NULL DEFAULT '[]'::jsonb;
