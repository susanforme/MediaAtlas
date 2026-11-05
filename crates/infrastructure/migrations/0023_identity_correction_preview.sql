-- Legacy previews retain their original conflict-only identity policy.
ALTER TABLE video_merge_previews ADD COLUMN expected_state_json TEXT
    CHECK(expected_state_json IS NULL OR json_valid(expected_state_json));
CREATE TRIGGER video_merge_identity_immutable BEFORE UPDATE OF expected_state_json ON video_merge_previews
BEGIN SELECT RAISE(ABORT,'immutable_video_merge_identity'); END;
