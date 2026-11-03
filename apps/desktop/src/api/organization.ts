import { request } from "./library";
export type OrganizationStrategy = "preserve" | "move" | "copy" | "hardlink";
export interface OrganizationRequest {
  media_item_id: string;
  target_root_id: string;
  template: string;
  strategy: OrganizationStrategy;
}
export interface OrganizationPlan {
  id: string;
  media_item_id: string;
  target_root_id: string;
  strategy: OrganizationStrategy;
  state: string;
  files: {
    source: string;
    target: string;
    role: string;
    bytes: number;
    conflict: string | null;
    same_path: boolean;
  }[];
  estimated_bytes: number;
  file_count: number;
  warnings: string[];
}
export interface OrganizationResult {
  id: string;
  state: string;
  completed: number;
  already_applied: boolean;
  error_code: string | null;
}
const object = (v: unknown): v is Record<string, unknown> =>
  v !== null && typeof v === "object" && !Array.isArray(v);
const text = (v: unknown): v is string => typeof v === "string";
const uint = (v: unknown): v is number =>
  typeof v === "number" && Number.isSafeInteger(v) && v >= 0;
const maybe = (v: unknown): v is string | null => v === null || text(v);
export function organizationPlan(v: unknown): v is OrganizationPlan {
  return (
    object(v) &&
    [v.id, v.media_item_id, v.target_root_id, v.state].every(text) &&
    ["preserve", "move", "copy", "hardlink"].includes(String(v.strategy)) &&
    uint(v.estimated_bytes) &&
    uint(v.file_count) &&
    Array.isArray(v.warnings) &&
    v.warnings.every(text) &&
    Array.isArray(v.files) &&
    v.files.every(
      (f) =>
        object(f) &&
        [f.source, f.target, f.role].every(text) &&
        uint(f.bytes) &&
        maybe(f.conflict) &&
        typeof f.same_path === "boolean",
    )
  );
}
function result(v: unknown): v is OrganizationResult {
  return (
    object(v) &&
    text(v.id) &&
    text(v.state) &&
    uint(v.completed) &&
    typeof v.already_applied === "boolean" &&
    maybe(v.error_code)
  );
}
export const organizationApi = (adult = false) => {
  const prefix = adult ? "adult_organization" : "organization";
  return {
    plan: (id: string) => request(`${prefix}_plan`, { id }, organizationPlan),
    logs: (id: string) =>
      request(
        `${prefix}_logs`,
        { id },
        (v): v is { file_index: number; stage: string; created_at: string }[] =>
          Array.isArray(v) &&
          v.every(
            (i) =>
              object(i) &&
              uint(i.file_index) &&
              text(i.stage) &&
              text(i.created_at),
          ),
      ),
    preview: (input: OrganizationRequest) =>
      request(`${prefix}_preview`, input, organizationPlan),
    execute: (id: string) => request(`${prefix}_execute`, { id }, result),
    cancel: (id: string) => request(`${prefix}_cancel`, { id }, result),
    discard: (id: string) =>
      request(`${prefix}_discard`, { id }, (v): v is null => v === null),
    rollbackPreview: (id: string) =>
      request(`${prefix}_rollback_preview`, { id }, organizationPlan),
    rollbackExecute: (id: string) =>
      request(`${prefix}_rollback_execute`, { id }, result),
    history: () =>
      request(
        `${prefix}_history`,
        {},
        (v): v is OrganizationResult[] => Array.isArray(v) && v.every(result),
      ),
  };
};
export interface WatcherStatus {
  root_id: string;
  opted_in: boolean;
  active: boolean;
  pending_rescan: boolean;
  last_task_id: string | null;
  error_code: string | null;
}
function watcher(v: unknown): v is WatcherStatus {
  return (
    object(v) &&
    text(v.root_id) &&
    [v.opted_in, v.active, v.pending_rescan].every(
      (x) => typeof x === "boolean",
    ) &&
    maybe(v.last_task_id) &&
    maybe(v.error_code)
  );
}
export interface HealthPage {
  items: {
    media_id: string;
    title: string;
    issues: string[];
    repair_route: string;
  }[];
  next_cursor: string | null;
}
export const maintenanceApi = {
  watchers: () =>
    request(
      "watcher_list",
      {},
      (v): v is WatcherStatus[] => Array.isArray(v) && v.every(watcher),
    ),
  configure: (rootId: string, enabled: boolean) =>
    request("watcher_configure", { root_id: rootId, enabled }, watcher),
  health: (cursor: string | null = null) =>
    request(
      "health_check",
      { cursor, limit: 50 },
      (v): v is HealthPage =>
        object(v) &&
        maybe(v.next_cursor) &&
        Array.isArray(v.items) &&
        v.items.every(
          (i) =>
            object(i) &&
            [i.media_id, i.title, i.repair_route].every(text) &&
            Array.isArray(i.issues) &&
            i.issues.every(text),
        ),
    ),
};
export const nativeOpenApi = {
  open: (fileId: string, adult = false) =>
    request(
      adult ? "adult_open" : "library_open",
      { file_id: fileId },
      (v): v is { requested: boolean } => object(v) && v.requested === true,
    ),
};
