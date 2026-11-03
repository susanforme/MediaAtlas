CREATE TABLE organization_operations (
 id TEXT PRIMARY KEY,
 lane TEXT NOT NULL CHECK(lane IN ('ordinary','adult')),
 kind TEXT NOT NULL CHECK(kind IN ('organize','rollback')),
 original_id TEXT REFERENCES organization_operations(id),
 media_item_id TEXT NOT NULL,
 plan_json TEXT NOT NULL,
 state TEXT NOT NULL,
 error_code TEXT,
 created_at TEXT NOT NULL,
 updated_at TEXT NOT NULL
);
CREATE TABLE organization_journal (
 sequence INTEGER PRIMARY KEY AUTOINCREMENT,
 operation_id TEXT NOT NULL REFERENCES organization_operations(id),
 file_index INTEGER NOT NULL,
 stage TEXT NOT NULL,
 stamp_json TEXT,
 created_at TEXT NOT NULL
);
CREATE INDEX organization_journal_operation ON organization_journal(operation_id,sequence);
CREATE TRIGGER organization_plan_immutable BEFORE UPDATE OF id,lane,kind,original_id,media_item_id,plan_json,created_at ON organization_operations
BEGIN SELECT RAISE(ABORT,'immutable_organization_plan'); END;
CREATE TRIGGER organization_journal_no_update BEFORE UPDATE ON organization_journal
BEGIN SELECT RAISE(ABORT,'immutable_organization_journal'); END;
CREATE TRIGGER organization_journal_no_delete BEFORE DELETE ON organization_journal
BEGIN SELECT RAISE(ABORT,'immutable_organization_journal'); END;
