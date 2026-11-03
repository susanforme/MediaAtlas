-- Published migrations are append-only. Type-specific business tables arrive later.
CREATE TABLE library_roots (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL CHECK(length(trim(name)) > 0),
    path TEXT NOT NULL,
    normalized_path TEXT NOT NULL CHECK(length(normalized_path) > 0),
    path_style TEXT NOT NULL CHECK(path_style IN ('windows','posix')),
    kind TEXT CHECK(kind IN ('movie','tv_series','tv_season','tv_episode','anime','adult_video','music_artist','music_release_group','music_release','music_track','music_recording','music_video','concert')),
    enabled INTEGER NOT NULL CHECK(enabled IN (0,1)),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL CHECK(updated_at >= created_at),
    UNIQUE(path_style, normalized_path)
);

CREATE TABLE media_items (
    id TEXT PRIMARY KEY NOT NULL,
    kind TEXT NOT NULL CHECK(kind IN ('movie','tv_series','tv_season','tv_episode','anime','adult_video','music_artist','music_release_group','music_release','music_track','music_recording','music_video','concert')),
    title TEXT NOT NULL CHECK(length(trim(title)) > 0),
    original_title TEXT,
    year INTEGER,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL CHECK(updated_at >= created_at)
);
CREATE INDEX media_items_kind_updated ON media_items(kind,updated_at DESC,id ASC);
CREATE INDEX media_items_updated ON media_items(updated_at DESC,id ASC);
CREATE INDEX media_items_title ON media_items(title);

CREATE TABLE media_files (
    id TEXT PRIMARY KEY NOT NULL,
    media_item_id TEXT NOT NULL REFERENCES media_items(id) ON DELETE CASCADE,
    root_id TEXT NOT NULL REFERENCES library_roots(id) ON DELETE CASCADE,
    relative_path TEXT NOT NULL,
    normalized_relative_path TEXT NOT NULL CHECK(length(normalized_relative_path) > 0),
    size_bytes INTEGER NOT NULL CHECK(size_bytes >= 0),
    modified_at INTEGER,
    part INTEGER CHECK(part >= 0 AND part <= 4294967295),
    UNIQUE(root_id,normalized_relative_path)
);
CREATE INDEX media_files_media_root ON media_files(media_item_id,root_id);

CREATE TABLE artworks (
    id TEXT PRIMARY KEY NOT NULL,
    media_item_id TEXT NOT NULL REFERENCES media_items(id) ON DELETE CASCADE,
    kind TEXT NOT NULL CHECK(kind IN ('poster','backdrop','logo','thumb','scene','artist','album_cover','cover','back_cover','sample')),
    cache_path TEXT,
    source_url TEXT,
    provider_id TEXT,
    width INTEGER CHECK(width > 0 AND width <= 4294967295),
    height INTEGER CHECK(height > 0 AND height <= 4294967295),
    content_hash TEXT,
    created_at INTEGER NOT NULL
);
CREATE INDEX artworks_media ON artworks(media_item_id);

CREATE TABLE external_ids (
    id TEXT PRIMARY KEY NOT NULL,
    media_item_id TEXT NOT NULL REFERENCES media_items(id) ON DELETE CASCADE,
    provider_id TEXT NOT NULL CHECK(length(trim(provider_id)) > 0),
    namespace TEXT NOT NULL CHECK(length(trim(namespace)) > 0),
    external_id TEXT NOT NULL CHECK(length(trim(external_id)) > 0),
    is_primary INTEGER NOT NULL CHECK(is_primary IN (0,1)),
    created_at INTEGER NOT NULL,
    UNIQUE(media_item_id,provider_id,namespace,external_id)
);
CREATE UNIQUE INDEX external_ids_primary ON external_ids(media_item_id,provider_id,namespace) WHERE is_primary=1;
CREATE INDEX external_ids_lookup ON external_ids(provider_id,namespace,external_id);

CREATE TABLE metadata_values (
    id TEXT PRIMARY KEY NOT NULL,
    media_item_id TEXT NOT NULL REFERENCES media_items(id) ON DELETE CASCADE,
    field_key TEXT NOT NULL CHECK(length(trim(field_key)) > 0),
    locale TEXT NOT NULL DEFAULT '',
    value_json TEXT NOT NULL CHECK(json_valid(value_json)),
    provider_id TEXT,
    source_url TEXT,
    fetched_at INTEGER,
    confidence REAL CHECK(confidence >= 0.0 AND confidence <= 1.0),
    is_user_modified INTEGER NOT NULL CHECK(is_user_modified IN (0,1)),
    is_locked INTEGER NOT NULL CHECK(is_locked IN (0,1)),
    UNIQUE(media_item_id,field_key,locale)
);
