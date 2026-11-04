ALTER TABLE root_settings ADD COLUMN metadata_providers_json TEXT
    CHECK(metadata_providers_json IS NULL OR
        (json_valid(metadata_providers_json) AND json_type(metadata_providers_json)='array'));
