-- IDs and bounded, typed return filters only. No source paths, credentials or write replay.
CREATE TABLE review_queues (
 id TEXT PRIMARY KEY NOT NULL,
 revision INTEGER NOT NULL DEFAULT 0 CHECK(revision>=0),
 current_media_id TEXT,
 context_json TEXT NOT NULL CHECK(json_valid(context_json)),
 preview_watermark INTEGER NOT NULL DEFAULT 0,
 created_at INTEGER NOT NULL,
 updated_at INTEGER NOT NULL
);
CREATE TABLE review_queue_items (
 queue_id TEXT NOT NULL REFERENCES review_queues(id) ON DELETE CASCADE,
 media_id TEXT NOT NULL,
 position INTEGER NOT NULL CHECK(position>=0 AND position<32),
 state TEXT NOT NULL CHECK(state IN ('pending','deferred','confirmed','failed')),
 checkpoint TEXT,
 preview_id TEXT UNIQUE,
 checked_at INTEGER NOT NULL,
 scope_hash TEXT NOT NULL,
 checkpoint_at INTEGER,
 PRIMARY KEY(queue_id,media_id),
 UNIQUE(queue_id,position)
);
CREATE INDEX review_queues_recent ON review_queues(updated_at DESC);
CREATE TABLE review_queue_checkpoints (
 preview_id TEXT PRIMARY KEY NOT NULL,
 queue_id TEXT NOT NULL REFERENCES review_queues(id),
 media_id TEXT NOT NULL,
 checkpoint TEXT UNIQUE NOT NULL,
 revision INTEGER NOT NULL,
 scope_hash TEXT NOT NULL,
 created_at INTEGER NOT NULL
);
CREATE TRIGGER review_checkpoint_immutable BEFORE UPDATE ON review_queue_checkpoints
BEGIN SELECT RAISE(ABORT,'immutable_review_checkpoint'); END;
-- Keep ledger entries after media/preview deletion so missing items are reviewable.
