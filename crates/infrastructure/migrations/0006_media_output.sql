CREATE TABLE output_previews (
    id TEXT PRIMARY KEY NOT NULL REFERENCES file_operations(id) ON DELETE CASCADE,
    media_item_id TEXT NOT NULL REFERENCES media_items(id) ON DELETE CASCADE,
    snapshot TEXT NOT NULL CHECK (json_valid(snapshot))
);
CREATE TRIGGER output_snapshot_immutable BEFORE UPDATE ON output_previews
BEGIN SELECT RAISE(ABORT, 'immutable_output_snapshot'); END;
