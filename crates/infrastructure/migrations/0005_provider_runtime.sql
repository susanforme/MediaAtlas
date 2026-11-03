-- Provider lifecycle extends the shared task/run audit without altering scan state.
CREATE TABLE provider_configs (
    provider_id TEXT PRIMARY KEY NOT NULL,
    config_json TEXT NOT NULL CHECK (json_valid(config_json))
);
CREATE TABLE provider_sessions (
    task_id TEXT PRIMARY KEY NOT NULL REFERENCES scrape_tasks(id) ON DELETE CASCADE,
    provider_id TEXT NOT NULL,
    job_json TEXT NOT NULL CHECK (json_valid(job_json)),
    failure TEXT,
    result_summary TEXT
);
CREATE INDEX provider_sessions_provider ON provider_sessions(provider_id);
CREATE TABLE provider_requests (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    task_id TEXT NOT NULL REFERENCES provider_sessions(task_id) ON DELETE CASCADE,
    record_json TEXT NOT NULL CHECK (json_valid(record_json))
);
CREATE INDEX provider_requests_task ON provider_requests(task_id);
CREATE TRIGGER provider_job_immutable BEFORE UPDATE OF task_id,provider_id,job_json ON provider_sessions
WHEN NEW.task_id IS NOT OLD.task_id OR NEW.provider_id IS NOT OLD.provider_id OR NEW.job_json IS NOT OLD.job_json
BEGIN SELECT RAISE(ABORT, 'immutable_provider_job'); END;
