-- Credential material belongs exclusively in native key storage or explicit RAM.
CREATE TABLE provider_auth_settings (
    provider_id TEXT PRIMARY KEY NOT NULL CHECK (provider_id = 'tmdb'),
    method TEXT NOT NULL CHECK (method IN ('bearer','api_key')),
    storage TEXT NOT NULL CHECK (storage IN ('system','session_only')),
    credential_ref TEXT NOT NULL UNIQUE
);
-- Durable cleanup intent is committed with each credential rotation/removal.
-- This outbox contains references only, never credential values.
CREATE TABLE provider_auth_retirements (
    credential_ref TEXT PRIMARY KEY NOT NULL,
    provider_id TEXT NOT NULL CHECK (provider_id = 'tmdb'),
    storage TEXT NOT NULL CHECK (storage IN ('system','session_only'))
);
CREATE INDEX provider_auth_retirements_provider ON provider_auth_retirements(provider_id);
