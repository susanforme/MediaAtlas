-- Immutable tag plans include full original bytes for recovery even if an external
-- process modifies the quarantine through an already-open descriptor.
CREATE TABLE music_tag_operations (
 id TEXT PRIMARY KEY,
 file_id TEXT NOT NULL REFERENCES media_files(id),
 kind TEXT NOT NULL CHECK(kind IN ('write','restore')),
 original_operation_id TEXT REFERENCES music_tag_operations(id),
 plan_json TEXT NOT NULL,
 before_bytes BLOB,
 payload BLOB NOT NULL,
 state TEXT NOT NULL CHECK(state IN ('preview','running','succeeded','failed','conflict','cancelled','interrupted')),
 error_code TEXT,
 created_at TEXT NOT NULL,
 updated_at TEXT NOT NULL
);
CREATE INDEX music_tag_file_history ON music_tag_operations(file_id,created_at,id);
CREATE TRIGGER music_tag_plan_immutable BEFORE UPDATE OF id,file_id,kind,original_operation_id,plan_json,before_bytes,payload,created_at ON music_tag_operations
BEGIN SELECT RAISE(ABORT,'immutable_music_tag_plan'); END;
