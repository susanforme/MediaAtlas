import { request } from "./library";

export interface BackupSummary {
  id: string;
  created_at: string;
  app_version: string;
  schema_version: number;
  bytes: number;
  contains_private_metadata: boolean;
}
export interface MaintenanceStatus {
  backups: BackupSummary[];
  restore_pending: boolean;
  directory: string;
}
export interface BackupPreview {
  id: string;
  contains_private_metadata: boolean;
  includes_secrets: false;
  includes_source_media: false;
  may_include_tag_recovery_bytes: true;
}
export interface RestorePreview {
  id: string;
  backup_id: string;
  contains_private_metadata: boolean;
  restart_required: true;
}
export interface CachePreview {
  id: string;
  files: number;
  bytes: number;
}
export interface ExportResult {
  id: string;
  path: string;
  bytes: number;
}
const object = (v: unknown): v is Record<string, unknown> =>
  typeof v === "object" && v !== null && !Array.isArray(v);
const text = (v: unknown): v is string => typeof v === "string";
const uint = (v: unknown): v is number =>
  typeof v === "number" && Number.isSafeInteger(v) && v >= 0;
const bool = (v: unknown): v is boolean => typeof v === "boolean";
function backup(v: unknown): v is BackupSummary {
  return (
    object(v) &&
    [v.id, v.created_at, v.app_version].every(text) &&
    uint(v.schema_version) &&
    uint(v.bytes) &&
    bool(v.contains_private_metadata)
  );
}
export const databaseMaintenanceApi = {
  status: () =>
    request(
      "maintenance_status",
      {},
      (v): v is MaintenanceStatus =>
        object(v) &&
        Array.isArray(v.backups) &&
        v.backups.every(backup) &&
        bool(v.restore_pending) &&
        text(v.directory),
    ),
  backupPreview: () =>
    request(
      "maintenance_backup_preview",
      {},
      (v): v is BackupPreview =>
        object(v) &&
        text(v.id) &&
        bool(v.contains_private_metadata) &&
        v.includes_secrets === false &&
        v.includes_source_media === false &&
        v.may_include_tag_recovery_bytes === true,
    ),
  backupConfirm: (id: string) =>
    request("maintenance_backup_confirm", { id }, backup),
  restorePreview: (id: string) =>
    request(
      "maintenance_restore_preview",
      { id },
      (v): v is RestorePreview =>
        object(v) &&
        text(v.id) &&
        text(v.backup_id) &&
        bool(v.contains_private_metadata) &&
        v.restart_required === true,
    ),
  restoreConfirm: (id: string) =>
    request(
      "maintenance_restore_confirm",
      { id },
      (v): v is { restart_required: true } =>
        object(v) && v.restart_required === true,
    ),
  diagnostics: () =>
    request(
      "maintenance_diagnostics_export",
      {},
      (v): v is ExportResult =>
        object(v) && text(v.id) && text(v.path) && uint(v.bytes),
    ),
  cachePreview: () =>
    request(
      "maintenance_cache_preview",
      {},
      (v): v is CachePreview =>
        object(v) && text(v.id) && uint(v.files) && uint(v.bytes),
    ),
  cacheConfirm: (id: string) =>
    request(
      "maintenance_cache_confirm",
      { id },
      (v): v is { removed_files: number } => object(v) && uint(v.removed_files),
    ),
  cancel: () => request("maintenance_cancel", {}, (v): v is null => v === null),
};
