-- Adult search and review snapshots remain separate from ordinary video workflows.
CREATE TABLE adult_scrape_sessions (
 task_id TEXT PRIMARY KEY NOT NULL REFERENCES scrape_tasks(id) ON DELETE CASCADE,
 query_json TEXT NOT NULL CHECK(json_valid(query_json)),
 jobs_json TEXT NOT NULL CHECK(json_valid(jobs_json)),
 search_json TEXT CHECK(search_json IS NULL OR json_valid(search_json)),
 created_at INTEGER NOT NULL
);
CREATE TABLE adult_scrape_details (
 task_id TEXT NOT NULL REFERENCES adult_scrape_sessions(task_id) ON DELETE CASCADE,
 candidate_index INTEGER NOT NULL CHECK(candidate_index >= 0),
 detail_json TEXT NOT NULL CHECK(json_valid(detail_json)),
 PRIMARY KEY(task_id,candidate_index)
);
CREATE TABLE adult_merge_previews (
 id TEXT PRIMARY KEY NOT NULL,
 task_id TEXT NOT NULL REFERENCES adult_scrape_sessions(task_id) ON DELETE CASCADE,
 media_item_id TEXT NOT NULL REFERENCES media_items(id) ON DELETE CASCADE,
 preview_json TEXT NOT NULL CHECK(json_valid(preview_json)),
 detail_json TEXT NOT NULL CHECK(json_valid(detail_json)),
 expected_media_json TEXT NOT NULL CHECK(json_valid(expected_media_json)),
 expected_roots_json TEXT NOT NULL CHECK(json_valid(expected_roots_json)),
 state TEXT NOT NULL CHECK(state IN('planned','running','succeeded','failed','interrupted')),
 result_json TEXT CHECK(result_json IS NULL OR json_valid(result_json)),
 created_at INTEGER NOT NULL
);
CREATE TABLE adult_metadata_sources (
 media_item_id TEXT PRIMARY KEY NOT NULL REFERENCES media_items(id) ON DELETE CASCADE,
 metadata_json TEXT NOT NULL CHECK(json_valid(metadata_json)),
 merge_preview_id TEXT NOT NULL REFERENCES adult_merge_previews(id),
 updated_at INTEGER NOT NULL
);
CREATE TRIGGER adult_merge_immutable BEFORE UPDATE OF id,task_id,media_item_id,preview_json,detail_json,expected_media_json,expected_roots_json,created_at ON adult_merge_previews
BEGIN SELECT RAISE(ABORT,'immutable_adult_merge'); END;
CREATE TRIGGER adult_detail_immutable BEFORE UPDATE ON adult_scrape_details
BEGIN SELECT RAISE(ABORT,'immutable_adult_detail'); END;
CREATE TRIGGER adult_session_inputs_immutable BEFORE UPDATE OF task_id,query_json,jobs_json,created_at ON adult_scrape_sessions
BEGIN SELECT RAISE(ABORT,'immutable_adult_session_inputs'); END;
CREATE TRIGGER adult_search_results_immutable BEFORE UPDATE OF search_json ON adult_scrape_sessions WHEN OLD.search_json IS NOT NULL
BEGIN SELECT RAISE(ABORT,'immutable_adult_search_results'); END;
