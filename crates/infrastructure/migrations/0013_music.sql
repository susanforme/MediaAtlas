-- Independent MusicBrainz graph. Relations are queryable and foreign-key checked;
-- JSON holds the lossless type-specific entity fields, never video metadata.
CREATE TABLE music_artists (
 media_item_id TEXT PRIMARY KEY REFERENCES media_items(id) ON DELETE CASCADE,
 entity_json TEXT NOT NULL CHECK(json_valid(entity_json))
);
CREATE TABLE music_release_groups (
 media_item_id TEXT PRIMARY KEY REFERENCES media_items(id) ON DELETE CASCADE,
 entity_json TEXT NOT NULL CHECK(json_valid(entity_json))
);
CREATE TABLE music_releases (
 media_item_id TEXT PRIMARY KEY REFERENCES media_items(id) ON DELETE CASCADE,
 release_group_id TEXT NOT NULL REFERENCES music_release_groups(media_item_id),
 entity_json TEXT NOT NULL CHECK(json_valid(entity_json)),
 source_url TEXT NOT NULL,
 fetched_at INTEGER NOT NULL,
 locale TEXT,
 release_date_text TEXT,
 release_disambiguation TEXT,
 group_first_release_date_text TEXT
);
CREATE INDEX music_releases_group ON music_releases(release_group_id);
CREATE TABLE music_media (
 id TEXT PRIMARY KEY NOT NULL,
 release_id TEXT NOT NULL REFERENCES music_releases(media_item_id) ON DELETE CASCADE,
 position INTEGER NOT NULL CHECK(position>0),
 entity_json TEXT NOT NULL CHECK(json_valid(entity_json)),
 UNIQUE(release_id,position)
);
CREATE TABLE music_recordings (
 media_item_id TEXT PRIMARY KEY REFERENCES media_items(id) ON DELETE CASCADE,
 entity_json TEXT NOT NULL CHECK(json_valid(entity_json))
);
CREATE TABLE music_tracks (
 media_item_id TEXT PRIMARY KEY REFERENCES media_items(id) ON DELETE CASCADE,
 medium_id TEXT NOT NULL REFERENCES music_media(id) ON DELETE CASCADE,
 recording_id TEXT NOT NULL REFERENCES music_recordings(media_item_id),
 position INTEGER NOT NULL CHECK(position>0),
 entity_json TEXT NOT NULL CHECK(json_valid(entity_json)),
 UNIQUE(medium_id,position)
);
CREATE INDEX music_tracks_recording ON music_tracks(recording_id);
CREATE TABLE music_artist_aliases (
 artist_id TEXT NOT NULL REFERENCES music_artists(media_item_id) ON DELETE CASCADE,
 position INTEGER NOT NULL CHECK(position>=0),
 alias TEXT NOT NULL,
 PRIMARY KEY(artist_id,position)
);
CREATE TABLE music_artist_credits (
 media_item_id TEXT NOT NULL REFERENCES media_items(id) ON DELETE CASCADE,
 position INTEGER NOT NULL CHECK(position>=0),
 artist_id TEXT NOT NULL REFERENCES music_artists(media_item_id),
 credited_name TEXT NOT NULL,
 join_phrase TEXT NOT NULL,
 PRIMARY KEY(media_item_id,position)
);
CREATE INDEX music_credits_artist ON music_artist_credits(artist_id,media_item_id);
CREATE TABLE music_identifiers (
 media_item_id TEXT NOT NULL REFERENCES media_items(id) ON DELETE CASCADE,
 kind TEXT NOT NULL CHECK(kind IN('music_brainz','isrc','upc','ean','catalog_number')),
 namespace TEXT NOT NULL,
 value TEXT NOT NULL,
 PRIMARY KEY(media_item_id,kind,namespace,value)
);
-- MBIDs are scoped by entity; barcode/ISRC values are deliberately not globally unique.
CREATE UNIQUE INDEX music_mbid_identity ON music_identifiers(namespace,value) WHERE kind='music_brainz';
CREATE TABLE music_track_files (
 track_id TEXT NOT NULL REFERENCES music_tracks(media_item_id) ON DELETE CASCADE,
 file_id TEXT NOT NULL REFERENCES media_files(id) ON DELETE CASCADE,
 PRIMARY KEY(track_id,file_id),
 UNIQUE(file_id)
);
