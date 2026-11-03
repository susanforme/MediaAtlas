CREATE TABLE scrape_tasks (
    id TEXT PRIMARY KEY NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('scan_library','parse_file','search_providers','fetch_metadata','download_artwork','merge_metadata','export_nfo','write_audio_tags','organize_files','refresh_library')),
    state TEXT NOT NULL CHECK (state IN ('queued','running','waiting_for_user','succeeded','failed','cancelled','interrupted')),
    input_snapshot TEXT NOT NULL CHECK (json_valid(input_snapshot)),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL CHECK (updated_at >= created_at)
);
CREATE INDEX tasks_state_updated ON scrape_tasks(state, updated_at);

CREATE TABLE scrape_runs (
    id TEXT PRIMARY KEY NOT NULL,
    task_id TEXT NOT NULL REFERENCES scrape_tasks(id) ON DELETE CASCADE,
    media_item_id TEXT REFERENCES media_items(id) ON DELETE SET NULL,
    provider_id TEXT,
    config_snapshot TEXT NOT NULL CHECK (json_valid(config_snapshot)),
    state TEXT NOT NULL CHECK (state IN ('queued','running','waiting_for_user','succeeded','failed','cancelled','interrupted')),
    started_at INTEGER NOT NULL,
    finished_at INTEGER CHECK (finished_at >= started_at)
);
CREATE INDEX runs_task ON scrape_runs(task_id);
CREATE INDEX runs_media ON scrape_runs(media_item_id);

CREATE TABLE file_operations (
    id TEXT PRIMARY KEY NOT NULL,
    task_id TEXT REFERENCES scrape_tasks(id) ON DELETE SET NULL,
    plan TEXT NOT NULL CHECK (json_valid(plan)),
    state TEXT NOT NULL CHECK (state IN ('planned','approved','running','succeeded','failed','rolled_back','interrupted')),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL CHECK (updated_at >= created_at)
);
CREATE INDEX operations_task ON file_operations(task_id);
CREATE INDEX operations_state_updated ON file_operations(state, updated_at);

CREATE TRIGGER task_input_immutable BEFORE UPDATE OF id, kind, input_snapshot, created_at ON scrape_tasks
WHEN NEW.id IS NOT OLD.id OR NEW.kind IS NOT OLD.kind OR NEW.input_snapshot IS NOT OLD.input_snapshot OR NEW.created_at IS NOT OLD.created_at
BEGIN SELECT RAISE(ABORT, 'immutable_task_input'); END;

CREATE TRIGGER run_input_immutable BEFORE UPDATE OF id, task_id, provider_id, config_snapshot, started_at ON scrape_runs
WHEN NEW.id IS NOT OLD.id OR NEW.task_id IS NOT OLD.task_id OR NEW.provider_id IS NOT OLD.provider_id OR NEW.config_snapshot IS NOT OLD.config_snapshot OR NEW.started_at IS NOT OLD.started_at
BEGIN SELECT RAISE(ABORT, 'immutable_run_input'); END;

CREATE TRIGGER operation_plan_immutable BEFORE UPDATE OF id, plan, created_at ON file_operations
WHEN NEW.id IS NOT OLD.id OR NEW.plan IS NOT OLD.plan OR NEW.created_at IS NOT OLD.created_at
BEGIN SELECT RAISE(ABORT, 'immutable_operation_plan'); END;
