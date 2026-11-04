-- Separate file-output outcome from identification; a sidecar failure never
-- invalidates successfully acquired metadata.
CREATE TABLE automatic_nfo_exports (
    media_item_id TEXT PRIMARY KEY NOT NULL REFERENCES media_items(id) ON DELETE CASCADE,
    state TEXT NOT NULL CHECK(state IN ('written','preserved','failed')),
    error_code TEXT,
    updated_at INTEGER NOT NULL
);
