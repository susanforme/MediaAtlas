-- Preserve saved credentials and cleanup references while enabling OMDb API keys.
CREATE TABLE provider_auth_settings_v3 (
    provider_id TEXT PRIMARY KEY NOT NULL CHECK (provider_id IN ('tmdb','omdb','dmm')),
    method TEXT NOT NULL,
    storage TEXT NOT NULL CHECK (storage IN ('system','session_only')),
    credential_ref TEXT NOT NULL UNIQUE,
    CHECK ((provider_id='tmdb' AND method IN ('bearer','api_key')) OR (provider_id='omdb' AND method='api_key') OR (provider_id='dmm' AND method='affiliate'))
);
INSERT INTO provider_auth_settings_v3 SELECT * FROM provider_auth_settings;
DROP TABLE provider_auth_settings;
ALTER TABLE provider_auth_settings_v3 RENAME TO provider_auth_settings;
CREATE TABLE provider_auth_retirements_v3 (
    credential_ref TEXT PRIMARY KEY NOT NULL,
    provider_id TEXT NOT NULL CHECK (provider_id IN ('tmdb','omdb','dmm')),
    storage TEXT NOT NULL CHECK (storage IN ('system','session_only'))
);
INSERT INTO provider_auth_retirements_v3 SELECT * FROM provider_auth_retirements;
DROP TABLE provider_auth_retirements;
ALTER TABLE provider_auth_retirements_v3 RENAME TO provider_auth_retirements;
CREATE INDEX provider_auth_retirements_provider ON provider_auth_retirements(provider_id);
