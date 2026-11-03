import { type MediaKind, request, type TaskState } from "./library";

export const providerFailures = [
  "timeout",
  "dns_error",
  "tls_error",
  "network_error",
  "http4xx",
  "http5xx",
  "rate_limited",
  "blocked",
  "captcha",
  "not_found",
  "parse_error",
  "identity_mismatch",
  "cancelled",
  "unsafe_target",
  "response_too_large",
  "invalid_configuration",
  "credentials_unavailable",
  "disabled",
  "cooling_down",
] as const;
export type ProviderFailure = (typeof providerFailures)[number];
export interface ProviderConfig {
  enabled: boolean;
  revision: number;
  timeout_ms: number;
  concurrency: number;
  image_concurrency: number;
  interval_ms: number;
  max_response_bytes: number;
  max_retries: number;
  credential_ref: string | null;
}
export interface ProviderView {
  descriptor: {
    id: string;
    name: string;
    description: string;
    website: string | null;
    terms_url: string | null;
    privacy_url: string | null;
    license: string;
    documentation_url: string | null;
    engine_types: string[];
    requires_api_key: boolean;
    requires_login: boolean;
    rate_limit_policy: string;
    attribution: string;
    data_usage_notes: string;
    media_kinds: MediaKind[];
    languages: string[];
    capabilities: string[];
    fixture: boolean;
  };
  config: ProviderConfig;
  health: string;
}
export interface ProviderTask {
  id: string;
  provider_id: string;
  state: TaskState;
  query: string;
  created_at: string;
  updated_at: string;
  failure: ProviderFailure | null;
  result_summary: string | null;
}
export interface NetworkRecord {
  task_id: string;
  provider_id: string;
  engine: string;
  method: string;
  safe_url: string;
  summary: string | null;
  duration_ms: number;
  status: number | null;
  failure: ProviderFailure | null;
  attempt: number;
}
export interface ProviderSnapshot {
  schema_version: 1;
  providers: ProviderView[];
  tasks: ProviderTask[];
  requests: NetworkRecord[];
}
const kinds: MediaKind[] = [
  "movie",
  "tv_series",
  "tv_season",
  "tv_episode",
  "anime",
  "adult_video",
  "music_artist",
  "music_release_group",
  "music_release",
  "music_track",
  "music_recording",
  "music_video",
  "concert",
];
const states: TaskState[] = [
  "queued",
  "running",
  "waiting_for_user",
  "succeeded",
  "failed",
  "cancelled",
  "interrupted",
];
function object(v: unknown): v is Record<string, unknown> {
  return v !== null && typeof v === "object" && !Array.isArray(v);
}
function text(v: unknown): v is string {
  return typeof v === "string" && v.length <= 4096;
}
function optionalText(v: unknown): v is string | null {
  return v === null || text(v);
}
function integer(v: unknown): v is number {
  return typeof v === "number" && Number.isSafeInteger(v) && v >= 0;
}
function timestamp(v: unknown): v is string {
  return (
    text(v) && /^\d{4}-\d\d-\d\dT/.test(v) && Number.isFinite(Date.parse(v))
  );
}
function array<T>(v: unknown, guard: (v: unknown) => v is T): v is T[] {
  return Array.isArray(v) && v.length <= 100 && v.every(guard);
}
function failure(v: unknown): v is ProviderFailure | null {
  return v === null || providerFailures.includes(v as ProviderFailure);
}
function config(v: unknown): v is ProviderConfig {
  return (
    object(v) &&
    typeof v.enabled === "boolean" &&
    [
      v.revision,
      v.timeout_ms,
      v.concurrency,
      v.image_concurrency,
      v.interval_ms,
      v.max_response_bytes,
      v.max_retries,
    ].every(integer) &&
    optionalText(v.credential_ref)
  );
}
function webUrl(v: unknown): v is string | null {
  if (v === null) return true;
  if (!text(v)) return false;
  try {
    const u = new URL(v);
    return u.protocol === "https:" && !u.username && !u.password;
  } catch {
    return false;
  }
}
function provider(v: unknown): v is ProviderView {
  if (!object(v) || !object(v.descriptor)) return false;
  const d = v.descriptor;
  return (
    [
      d.id,
      d.name,
      d.description,
      d.license,
      d.rate_limit_policy,
      d.attribution,
      d.data_usage_notes,
    ].every(text) &&
    typeof d.requires_api_key === "boolean" &&
    typeof d.requires_login === "boolean" &&
    array(d.engine_types, text) &&
    [d.website, d.terms_url, d.privacy_url, d.documentation_url].every(
      webUrl,
    ) &&
    array(d.media_kinds, (x): x is MediaKind =>
      kinds.includes(x as MediaKind),
    ) &&
    array(d.languages, text) &&
    array(d.capabilities, text) &&
    typeof d.fixture === "boolean" &&
    config(v.config) &&
    text(v.health)
  );
}
function task(v: unknown): v is ProviderTask {
  return (
    object(v) &&
    [v.id, v.provider_id, v.query].every(text) &&
    states.includes(v.state as TaskState) &&
    timestamp(v.created_at) &&
    timestamp(v.updated_at) &&
    failure(v.failure) &&
    optionalText(v.result_summary)
  );
}
function safeUrl(v: unknown): v is string {
  if (v === "[redacted]") return true;
  if (!text(v)) return false;
  try {
    const u = new URL(v);
    return (
      ["http:", "https:", "fixture:"].includes(u.protocol) &&
      Boolean(u.hostname) &&
      !u.username &&
      !u.password &&
      !u.search &&
      !u.hash &&
      ["", "/"].includes(u.pathname)
    );
  } catch {
    return false;
  }
}
function record(v: unknown): v is NetworkRecord {
  return (
    object(v) &&
    [v.task_id, v.provider_id, v.engine].every(text) &&
    ["GET", "POST", "HEAD", "FIXTURE"].includes(String(v.method)) &&
    safeUrl(v.safe_url) &&
    integer(v.duration_ms) &&
    (v.status === null ||
      (integer(v.status) && v.status >= 100 && v.status <= 599)) &&
    failure(v.failure) &&
    optionalText(v.summary) &&
    integer(v.attempt)
  );
}
export function providerSnapshot(v: unknown): v is ProviderSnapshot {
  return (
    object(v) &&
    v.schema_version === 1 &&
    array(v.providers, provider) &&
    array(v.tasks, task) &&
    array(v.requests, record)
  );
}
export const providerApi = {
  snapshot: () => request("provider_snapshot", {}, providerSnapshot),
  updateConfig: (providerId: string, value: ProviderConfig) =>
    request(
      "provider_update_config",
      { provider_id: providerId, config: value },
      (v): v is null => v === null,
    ),
  setEnabled: (providerId: string, enabled: boolean) =>
    request(
      "provider_set_enabled",
      { provider_id: providerId, enabled },
      (v): v is null => v === null,
    ),
  start: (providerId: string, query: string, language: "zh-CN" | "en-US") =>
    request(
      "provider_start",
      { provider_id: providerId, query, language },
      text,
    ),
  cancel: (taskId: string) =>
    request(
      "provider_cancel",
      { task_id: taskId },
      (v): v is boolean => typeof v === "boolean",
    ),
};
