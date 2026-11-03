-- Configuration persists, native handles and pending event queues never do.
CREATE TABLE watcher_roots (
    root_id TEXT PRIMARY KEY NOT NULL REFERENCES library_roots(id) ON DELETE CASCADE,
    opted_in INTEGER NOT NULL DEFAULT 0 CHECK(opted_in IN (0,1)),
    last_task_id TEXT,
    updated_at INTEGER NOT NULL
);
