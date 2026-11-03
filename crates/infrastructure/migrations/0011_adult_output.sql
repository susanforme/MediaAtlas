-- Adult output is independently scoped and never replayed on startup.
CREATE TABLE adult_output_batches (
 id TEXT PRIMARY KEY NOT NULL,
 task_id TEXT NOT NULL UNIQUE REFERENCES scrape_tasks(id) ON DELETE CASCADE,
 snapshot TEXT NOT NULL CHECK(json_valid(snapshot)),
 state TEXT NOT NULL CHECK(state IN ('planned','running','succeeded','failed','cancelled','interrupted')),
 created_at INTEGER NOT NULL,
 updated_at INTEGER NOT NULL
);
CREATE TABLE adult_output_batch_items (
 batch_id TEXT NOT NULL REFERENCES adult_output_batches(id) ON DELETE CASCADE,
 ordinal INTEGER NOT NULL,
 media_item_id TEXT NOT NULL REFERENCES media_items(id),
 preview_id TEXT NOT NULL UNIQUE REFERENCES output_previews(id),
 state TEXT NOT NULL CHECK(state IN ('planned','running','succeeded','failed','cancelled','interrupted')),
 result_json TEXT CHECK(result_json IS NULL OR json_valid(result_json)),
 error_code TEXT,
 PRIMARY KEY(batch_id, ordinal)
);
CREATE TRIGGER adult_output_batch_immutable BEFORE UPDATE OF id,task_id,snapshot,created_at ON adult_output_batches
BEGIN SELECT RAISE(ABORT,'immutable_adult_output_batch'); END;
CREATE TRIGGER adult_output_item_immutable BEFORE UPDATE OF batch_id,ordinal,media_item_id,preview_id ON adult_output_batch_items
BEGIN SELECT RAISE(ABORT,'immutable_adult_output_item'); END;
