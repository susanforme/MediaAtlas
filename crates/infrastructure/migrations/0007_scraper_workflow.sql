CREATE TABLE video_scrape_sessions (
 task_id TEXT PRIMARY KEY NOT NULL REFERENCES scrape_tasks(id) ON DELETE CASCADE,
 query_json TEXT NOT NULL CHECK(json_valid(query_json)),
 jobs_json TEXT NOT NULL CHECK(json_valid(jobs_json)),
 search_json TEXT CHECK(search_json IS NULL OR json_valid(search_json)),
 created_at INTEGER NOT NULL
);
CREATE TABLE video_scrape_details (
 task_id TEXT NOT NULL REFERENCES video_scrape_sessions(task_id) ON DELETE CASCADE,
 candidate_index INTEGER NOT NULL,
 detail_json TEXT NOT NULL CHECK(json_valid(detail_json)),
 PRIMARY KEY(task_id,candidate_index)
);
CREATE TABLE video_merge_previews (
 id TEXT PRIMARY KEY NOT NULL,
 task_id TEXT NOT NULL REFERENCES video_scrape_sessions(task_id) ON DELETE CASCADE,
 media_item_id TEXT NOT NULL REFERENCES media_items(id) ON DELETE CASCADE,
 preview_json TEXT NOT NULL CHECK(json_valid(preview_json)),
 detail_json TEXT NOT NULL CHECK(json_valid(detail_json)),
 selected_artwork TEXT NOT NULL CHECK(json_valid(selected_artwork)),
 expected_media_json TEXT NOT NULL CHECK(json_valid(expected_media_json)),
 state TEXT NOT NULL CHECK(state IN('planned','running','succeeded','failed','interrupted')),
 result_json TEXT CHECK(result_json IS NULL OR json_valid(result_json)),
 created_at INTEGER NOT NULL
);
CREATE TRIGGER video_merge_immutable BEFORE UPDATE OF id,task_id,media_item_id,preview_json,detail_json,selected_artwork,expected_media_json,created_at ON video_merge_previews
BEGIN SELECT RAISE(ABORT,'immutable_video_merge'); END;
CREATE TRIGGER video_detail_immutable BEFORE UPDATE ON video_scrape_details
BEGIN SELECT RAISE(ABORT,'immutable_video_detail'); END;
-- Confirmed typed source snapshots keep TV/Anime structure available from the library.
CREATE TABLE video_metadata_sources (
 media_item_id TEXT PRIMARY KEY NOT NULL REFERENCES media_items(id) ON DELETE CASCADE,
 media_kind TEXT NOT NULL CHECK(media_kind IN ('movie','tv_series','anime')),
 metadata_json TEXT NOT NULL CHECK(json_valid(metadata_json)),
 merge_preview_id TEXT NOT NULL REFERENCES video_merge_previews(id),
 updated_at INTEGER NOT NULL
);
CREATE TRIGGER video_session_inputs_immutable BEFORE UPDATE OF task_id,query_json,jobs_json,created_at ON video_scrape_sessions
BEGIN SELECT RAISE(ABORT,'immutable_video_session_inputs'); END;
CREATE TRIGGER video_search_results_immutable BEFORE UPDATE OF search_json ON video_scrape_sessions
WHEN OLD.search_json IS NOT NULL
BEGIN SELECT RAISE(ABORT,'immutable_video_search_results'); END;
