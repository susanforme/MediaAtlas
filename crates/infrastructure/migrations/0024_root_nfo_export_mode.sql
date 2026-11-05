-- Choosing offline explicitly authorizes artwork sidecars for this ordinary-video root.
ALTER TABLE root_settings ADD COLUMN nfo_export_mode TEXT NOT NULL DEFAULT 'online'
    CHECK(nfo_export_mode IN ('online','offline'));
