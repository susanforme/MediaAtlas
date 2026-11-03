-- Explicit provenance is separate from inferred filename metadata. A confirmed
-- bare scan is a tool candidate, never evidence of completed metadata.
CREATE TABLE local_metadata_provenance (
    media_item_id TEXT PRIMARY KEY REFERENCES media_items(id) ON DELETE CASCADE,
    source_kind TEXT NOT NULL CHECK(source_kind IN ('nfo','audio_tags')),
    source_reference TEXT NOT NULL,
    confirmed_at INTEGER NOT NULL
);
