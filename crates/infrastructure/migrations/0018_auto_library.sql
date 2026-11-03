ALTER TABLE scan_sessions ADD COLUMN mode TEXT NOT NULL DEFAULT 'manual' CHECK(mode IN ('manual','automatic'));
ALTER TABLE scan_sessions ADD COLUMN phase TEXT NOT NULL DEFAULT 'scan' CHECK(phase IN ('scan','index','metadata','complete'));
ALTER TABLE scan_sessions ADD COLUMN matched_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE scan_sessions ADD COLUMN review_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE scan_sessions ADD COLUMN metadata_failed_count INTEGER NOT NULL DEFAULT 0;

-- The local file is browseable independently of its metadata enrichment result.
CREATE TABLE library_refresh_items (
    task_id TEXT NOT NULL REFERENCES scan_sessions(task_id) ON DELETE CASCADE,
    media_item_id TEXT NOT NULL REFERENCES media_items(id) ON DELETE CASCADE,
    state TEXT NOT NULL CHECK(state IN ('pending','matched','needs_review','failed','skipped')),
    reason TEXT,
    run_id TEXT REFERENCES scrape_tasks(id) ON DELETE SET NULL,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY(task_id,media_item_id)
);
CREATE INDEX library_refresh_media_state ON library_refresh_items(media_item_id,updated_at DESC,task_id DESC);
