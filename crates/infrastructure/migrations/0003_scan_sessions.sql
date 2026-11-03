CREATE TABLE scan_sessions (
    task_id TEXT PRIMARY KEY NOT NULL REFERENCES scrape_tasks(id) ON DELETE CASCADE,
    root_id TEXT NOT NULL REFERENCES library_roots(id) ON DELETE CASCADE,
    root_snapshot_json TEXT NOT NULL CHECK(json_valid(root_snapshot_json)),
    visited_files INTEGER NOT NULL DEFAULT 0,
    skipped_files INTEGER NOT NULL DEFAULT 0,
    candidate_count INTEGER NOT NULL DEFAULT 0,
    imported_count INTEGER NOT NULL DEFAULT 0,
    warnings INTEGER NOT NULL DEFAULT 0,
    error_code TEXT
);
CREATE INDEX scan_sessions_root ON scan_sessions(root_id);

CREATE TABLE scan_candidates (
    id TEXT PRIMARY KEY NOT NULL,
    task_id TEXT NOT NULL REFERENCES scan_sessions(task_id) ON DELETE CASCADE,
    root_id TEXT NOT NULL REFERENCES library_roots(id) ON DELETE CASCADE,
    kind TEXT NOT NULL,
    relative_path TEXT NOT NULL,
    normalized_relative_path TEXT NOT NULL,
    payload_json TEXT NOT NULL CHECK(json_valid(payload_json)),
    status TEXT NOT NULL CHECK(status IN ('pending','unchanged','imported')),
    existing_media_id TEXT REFERENCES media_items(id) ON DELETE SET NULL,
    UNIQUE(task_id, normalized_relative_path)
);
CREATE INDEX scan_candidates_task_page ON scan_candidates(task_id,id);
CREATE INDEX scan_candidates_root_path ON scan_candidates(root_id,normalized_relative_path);

CREATE TABLE scan_media_groups (
    root_id TEXT NOT NULL REFERENCES library_roots(id) ON DELETE CASCADE,
    group_key TEXT NOT NULL,
    media_item_id TEXT NOT NULL REFERENCES media_items(id) ON DELETE CASCADE,
    PRIMARY KEY(root_id,group_key)
);

CREATE TABLE media_relations (
    parent_id TEXT NOT NULL REFERENCES media_items(id) ON DELETE CASCADE,
    child_id TEXT NOT NULL REFERENCES media_items(id) ON DELETE CASCADE,
    relation_type TEXT NOT NULL CHECK(relation_type IN ('season','episode','anime_base')),
    PRIMARY KEY(parent_id,child_id,relation_type),
    CHECK(parent_id != child_id)
);
CREATE INDEX media_relations_child ON media_relations(child_id);

CREATE TABLE scan_source_fingerprints (
    root_id TEXT NOT NULL REFERENCES library_roots(id) ON DELETE CASCADE,
    normalized_relative_path TEXT NOT NULL,
    signature TEXT NOT NULL,
    PRIMARY KEY(root_id,normalized_relative_path)
);
