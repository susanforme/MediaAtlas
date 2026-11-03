import { invoke, isTauri } from "@tauri-apps/api/core";
import { DesktopError, type IpcErrorEnvelope } from "./desktop";

export type MediaKind =
  | "movie"
  | "tv_series"
  | "tv_season"
  | "tv_episode"
  | "anime"
  | "adult_video"
  | "music_artist"
  | "music_release_group"
  | "music_release"
  | "music_track"
  | "music_recording"
  | "music_video"
  | "concert";
export interface MediaItem {
  id: string;
  kind: MediaKind;
  title: string;
  original_title: string | null;
  year: number | null;
  created_at: string;
  updated_at: string;
}
export interface RootView {
  id: string;
  name: string;
  path: string;
  path_style: "windows" | "posix";
  kind: MediaKind | null;
  enabled: boolean;
  created_at: string;
  updated_at: string;
  exclude_patterns: string[];
  last_scan_at: string | null;
}
export interface RootInput {
  id: string | null;
  name: string;
  path: string;
  kind: MediaKind | null;
  enabled: boolean;
  exclude_patterns: string[];
}
export interface BrowseQuery {
  root_id: string | null;
  kind: MediaKind | null;
  query: string | null;
  health: string | null;
  genre: string | null;
  person: string | null;
  studio: string | null;
  year: number | null;
  sort: string;
  cursor: string | null;
  limit: number;
  include_adult: boolean;
}
export interface MediaCard {
  media: MediaItem;
  poster_uri: string | null;
  health: string[];
}
export interface MediaPage {
  items: MediaCard[];
  next_cursor: string | null;
}
export interface MediaFile {
  id: string;
  media_item_id: string;
  root_id: string;
  relative_path: string;
  size_bytes: number;
  modified_at: string | null;
  part: number | null;
}
export interface Artwork {
  id: string;
  kind: string;
  cache_path: string | null;
  source_url: string | null;
}
export interface ExternalId {
  id: string;
  provider_id: string;
  namespace: string;
  external_id: string;
}
export interface MetadataValue {
  id: string;
  field_key: string;
  locale: string | null;
  value: unknown;
  provider_id: string | null;
  source_url: string | null;
  is_user_modified: boolean;
  is_locked: boolean;
}
export interface MediaDetail {
  media: MediaItem;
  files: MediaFile[];
  artworks: Artwork[];
  external_ids: ExternalId[];
  metadata_values: MetadataValue[];
  relations: { id: string; kind: MediaKind; title: string }[];
  health: string[];
  poster_uri: string | null;
}
export interface Summary {
  total_media: number;
  total_roots: number;
  pending_tasks: number;
  failed_tasks: number;
  missing_poster: number;
  missing_nfo: number;
  incomplete_metadata: number;
}
export interface FacetOption {
  value: string;
  count: number;
}
export interface Facets {
  genres: FacetOption[];
  people: FacetOption[];
  studios: FacetOption[];
  years: FacetOption[];
}
export type TaskState =
  | "queued"
  | "running"
  | "waiting_for_user"
  | "succeeded"
  | "failed"
  | "cancelled"
  | "interrupted";
export interface ScanTask {
  id: string;
  root_id: string;
  state: TaskState;
  visited_files: number;
  skipped_files: number;
  candidate_count: number;
  imported_count: number;
  warnings: number;
  error_code: string | null;
  created_at: string;
  updated_at: string;
}
export interface ScanCandidate {
  id: string;
  root_id: string;
  relative_path: string;
  size_bytes: number;
  modified_at: string | null;
  kind: MediaKind;
  title: string;
  year: number | null;
  part: number | null;
  series_title: string | null;
  season_number: number | null;
  episode_number: number | null;
  absolute_number: number | null;
  catalog_number: string | null;
  needs_review: boolean;
  warnings: string[];
  existing_media_id: string | null;
  status: string;
}
export interface PreviewPage {
  task_id: string;
  items: ScanCandidate[];
  next_cursor: string | null;
}
export interface ConfirmResult {
  imported: number;
  unchanged: number;
  needs_review: number;
}

const MEDIA_KINDS: MediaKind[] = [
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
function record(v: unknown): v is Record<string, unknown> {
  return v !== null && typeof v === "object" && !Array.isArray(v);
}
function arrayOf<T>(guard: (v: unknown) => v is T) {
  return (v: unknown): v is T[] => Array.isArray(v) && v.every(guard);
}
function text(v: unknown): v is string {
  return typeof v === "string";
}
function optionalText(v: unknown) {
  return v === null || text(v);
}
function uint(v: unknown) {
  return (
    typeof v === "number" && Number.isFinite(v) && Number.isInteger(v) && v >= 0
  );
}
function optionalNumber(v: unknown) {
  return v === null || (typeof v === "number" && Number.isInteger(v));
}
function kind(v: unknown) {
  return MEDIA_KINDS.includes(v as MediaKind);
}
function poster(v: unknown) {
  return v === null || (text(v) && v.startsWith("data:image/png;base64,"));
}
function root(v: unknown): v is RootView {
  return (
    record(v) &&
    text(v.id) &&
    text(v.name) &&
    text(v.path) &&
    ["windows", "posix"].includes(String(v.path_style)) &&
    (v.kind === null || kind(v.kind)) &&
    typeof v.enabled === "boolean" &&
    arrayOf(text)(v.exclude_patterns) &&
    text(v.created_at) &&
    text(v.updated_at) &&
    optionalText(v.last_scan_at)
  );
}
function media(v: unknown): v is MediaItem {
  return (
    record(v) &&
    text(v.id) &&
    text(v.title) &&
    kind(v.kind) &&
    optionalText(v.original_title) &&
    optionalNumber(v.year) &&
    text(v.created_at) &&
    text(v.updated_at)
  );
}
function card(v: unknown): v is MediaCard {
  return (
    record(v) &&
    media(v.media) &&
    arrayOf(text)(v.health) &&
    poster(v.poster_uri)
  );
}
function task(v: unknown): v is ScanTask {
  return (
    record(v) &&
    text(v.id) &&
    text(v.root_id) &&
    [
      "queued",
      "running",
      "waiting_for_user",
      "succeeded",
      "failed",
      "cancelled",
      "interrupted",
    ].includes(String(v.state)) &&
    [
      v.visited_files,
      v.skipped_files,
      v.candidate_count,
      v.imported_count,
      v.warnings,
    ].every(uint) &&
    optionalText(v.error_code) &&
    text(v.created_at) &&
    text(v.updated_at)
  );
}
function candidate(v: unknown): v is ScanCandidate {
  return (
    record(v) &&
    text(v.id) &&
    text(v.root_id) &&
    text(v.relative_path) &&
    uint(v.size_bytes) &&
    optionalText(v.modified_at) &&
    kind(v.kind) &&
    text(v.title) &&
    [
      v.year,
      v.part,
      v.season_number,
      v.episode_number,
      v.absolute_number,
    ].every(optionalNumber) &&
    optionalText(v.series_title) &&
    optionalText(v.catalog_number) &&
    typeof v.needs_review === "boolean" &&
    arrayOf(text)(v.warnings) &&
    optionalText(v.existing_media_id) &&
    ["pending", "unchanged", "imported"].includes(String(v.status))
  );
}
function file(v: unknown): v is MediaFile {
  return (
    record(v) &&
    text(v.id) &&
    text(v.media_item_id) &&
    text(v.root_id) &&
    text(v.relative_path) &&
    uint(v.size_bytes) &&
    optionalText(v.modified_at) &&
    optionalNumber(v.part)
  );
}
function artwork(v: unknown): v is Artwork {
  return (
    record(v) &&
    text(v.id) &&
    text(v.kind) &&
    optionalText(v.cache_path) &&
    optionalText(v.source_url)
  );
}
function external(v: unknown): v is ExternalId {
  return (
    record(v) && [v.id, v.provider_id, v.namespace, v.external_id].every(text)
  );
}
function metadata(v: unknown): v is MetadataValue {
  return (
    record(v) &&
    text(v.id) &&
    text(v.field_key) &&
    optionalText(v.locale) &&
    "value" in v &&
    optionalText(v.provider_id) &&
    optionalText(v.source_url) &&
    typeof v.is_user_modified === "boolean" &&
    typeof v.is_locked === "boolean"
  );
}
function relation(v: unknown): v is MediaDetail["relations"][number] {
  return record(v) && text(v.id) && kind(v.kind) && text(v.title);
}
function detail(v: unknown): v is MediaDetail {
  return (
    record(v) &&
    media(v.media) &&
    arrayOf(file)(v.files) &&
    arrayOf(artwork)(v.artworks) &&
    arrayOf(external)(v.external_ids) &&
    arrayOf(metadata)(v.metadata_values) &&
    arrayOf(relation)(v.relations) &&
    arrayOf(text)(v.health) &&
    poster(v.poster_uri)
  );
}
function facet(v: unknown): v is FacetOption {
  return record(v) && text(v.value) && uint(v.count);
}
function facets(v: unknown): v is Facets {
  return (
    record(v) && [v.genres, v.people, v.studios, v.years].every(arrayOf(facet))
  );
}
function envelope(v: unknown): v is IpcErrorEnvelope {
  return (
    record(v) &&
    v.schema_version === 1 &&
    text(v.code) &&
    text(v.message) &&
    text(v.correlation_id)
  );
}
export async function request<T>(
  command: string,
  input: unknown,
  guard: (value: unknown) => value is T,
): Promise<T> {
  if (!isTauri()) throw new DesktopError("desktop_required");
  let response: unknown;
  try {
    response = await invoke(command, { input });
  } catch (error: unknown) {
    throw new DesktopError(
      "request_failed",
      envelope(error) ? error : undefined,
    );
  }
  if (
    !record(response) ||
    response.schema_version !== 1 ||
    !guard(response.data)
  )
    throw new DesktopError("invalid_response");
  return response.data;
}
export const defaultBrowse = (includeAdult = false): BrowseQuery => ({
  root_id: null,
  kind: null,
  query: null,
  health: null,
  genre: null,
  person: null,
  studio: null,
  year: null,
  sort: "updated_desc",
  cursor: null,
  limit: 48,
  include_adult: includeAdult,
});
export const libraryApi = {
  roots: (includeAdult: boolean) =>
    request(
      "library_list_roots",
      { include_adult: includeAdult },
      arrayOf(root),
    ),
  saveRoot: (input: RootInput) => request("library_save_root", input, root),
  removeRoot: (rootId: string) =>
    request(
      "library_remove_root",
      { root_id: rootId },
      (v): v is boolean => typeof v === "boolean",
    ),
  media: (input: BrowseQuery) =>
    request(
      "library_list_media",
      input,
      (v): v is MediaPage =>
        record(v) &&
        arrayOf(card)(v.items) &&
        (v.next_cursor === null || typeof v.next_cursor === "string"),
    ),
  detail: (mediaId: string, includeAdult: boolean) =>
    request(
      "library_get_media",
      { media_id: mediaId, include_adult: includeAdult },
      detail,
    ),
  summary: (input: BrowseQuery) =>
    request(
      "library_summary",
      input,
      (v): v is Summary =>
        record(v) &&
        [
          "total_media",
          "total_roots",
          "pending_tasks",
          "failed_tasks",
          "missing_poster",
          "missing_nfo",
          "incomplete_metadata",
        ].every((key) => uint(v[key])),
    ),
  facets: (input: BrowseQuery) => request("library_facets", input, facets),
  start: (rootId: string) => request("scan_start", { root_id: rootId }, task),
  tasks: (includeAdult: boolean) =>
    request("scan_list_tasks", { include_adult: includeAdult }, arrayOf(task)),
  preview: (taskId: string, cursor: string | null, includeAdult: boolean) =>
    request(
      "scan_get_preview",
      { task_id: taskId, cursor, limit: 50, include_adult: includeAdult },
      (v): v is PreviewPage =>
        record(v) &&
        typeof v.task_id === "string" &&
        arrayOf(candidate)(v.items) &&
        (v.next_cursor === null || typeof v.next_cursor === "string"),
    ),
  confirm: (taskId: string, candidateIds: string[]) =>
    request(
      "scan_confirm",
      { task_id: taskId, candidate_ids: candidateIds },
      (v): v is ConfirmResult =>
        record(v) &&
        uint(v.imported) &&
        uint(v.unchanged) &&
        uint(v.needs_review),
    ),
  cancel: (taskId: string) => request("scan_cancel", { task_id: taskId }, task),
};

// Reused by the separate adult IPC channel; ordinary visibility behavior is unchanged.
export {
  card as mediaCardGuard,
  detail as mediaDetailGuard,
  root as rootViewGuard,
  task as scanTaskGuard,
};
