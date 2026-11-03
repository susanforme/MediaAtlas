CREATE TABLE adult_privacy_settings (
 id INTEGER PRIMARY KEY CHECK(id=1),
 entry_visible INTEGER NOT NULL DEFAULT 0 CHECK(entry_visible IN(0,1)),
 covers_visible INTEGER NOT NULL DEFAULT 0 CHECK(covers_visible IN(0,1)),
 remember_recent INTEGER NOT NULL DEFAULT 0 CHECK(remember_recent IN(0,1))
);
INSERT INTO adult_privacy_settings(id) VALUES(1);
CREATE TABLE adult_recent_visits (
 media_item_id TEXT PRIMARY KEY NOT NULL REFERENCES media_items(id) ON DELETE CASCADE,
 visited_at INTEGER NOT NULL
);
-- Lock invalidates unfinished output intents even after a later fresh unlock.
CREATE TABLE adult_revoked_output_previews (
 preview_id TEXT PRIMARY KEY NOT NULL REFERENCES output_previews(id) ON DELETE CASCADE
);
