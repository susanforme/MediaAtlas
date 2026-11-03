CREATE TABLE root_settings (
    root_id TEXT PRIMARY KEY NOT NULL REFERENCES library_roots(id) ON DELETE CASCADE,
    exclude_patterns_json TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(exclude_patterns_json)),
    last_scan_at INTEGER
);
CREATE INDEX media_items_title_cursor ON media_items(title COLLATE NOCASE,id);
CREATE INDEX media_items_year_cursor ON media_items(year DESC,id);
CREATE INDEX media_items_added_cursor ON media_items(created_at DESC,id);

-- Keep content isolated after an Adult Root is removed or reclassified. Removing
-- a root preserves logical media records, so visibility cannot depend on files alone.
ALTER TABLE media_items ADD COLUMN adult_isolated INTEGER NOT NULL DEFAULT 0 CHECK(adult_isolated IN (0,1));
UPDATE media_items SET adult_isolated=1 WHERE kind='adult_video' OR EXISTS (
    SELECT 1 FROM media_files f JOIN library_roots r ON r.id=f.root_id
    WHERE f.media_item_id=media_items.id AND r.kind='adult_video'
);
CREATE TRIGGER isolate_adult_root_file AFTER INSERT ON media_files
WHEN (SELECT kind FROM library_roots WHERE id=NEW.root_id)='adult_video'
BEGIN UPDATE media_items SET adult_isolated=1 WHERE id=NEW.media_item_id; END;
CREATE TRIGGER isolate_reclassified_root AFTER UPDATE OF kind ON library_roots
WHEN NEW.kind='adult_video'
BEGIN UPDATE media_items SET adult_isolated=1 WHERE id IN (SELECT media_item_id FROM media_files WHERE root_id=NEW.id); END;
