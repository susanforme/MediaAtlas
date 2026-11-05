import { type MetadataValue, request } from "./library";
import { type ProviderFailure, providerFailures } from "./providers";

export type VideoKind = "movie" | "tv" | "anime";
export interface RemoteId {
  namespace: string;
  value: string;
}
interface QueryFields {
  title: string;
  year: number | null;
  external_ids: RemoteId[];
  language: string;
}
export interface MovieQuery extends QueryFields {
  kind: "movie";
}
export interface TvQuery extends QueryFields {
  kind: "tv";
}
export interface AnimeQuery extends QueryFields {
  kind: "anime";
}
export type VideoQuery = MovieQuery | TvQuery | AnimeQuery;
export interface SearchRequest {
  query: VideoQuery;
  providers: string[];
}
export interface CandidateData {
  provider_id: string;
  remote_id: string;
  title: string;
  original_title: string | null;
  year: number | null;
  aliases: string[];
  external_ids: RemoteId[];
  source_url: string;
  language: string;
}
export type VideoCandidate = { kind: VideoKind; data: CandidateData };
export interface MatchedCandidate {
  candidate: VideoCandidate;
  reasons: string[];
  needs_review: boolean;
  duplicate_of: number | null;
}
export interface SearchPreview {
  schema_version: 1;
  run_id: string;
  candidates: MatchedCandidate[];
  failures: { provider_id: string; failure: ProviderFailure }[];
}
export interface RemoteField {
  key: string;
  value: unknown;
  language: string | null;
}
export interface ArtworkCandidate {
  url: string;
  source_provider: string;
  kind: string;
  language: string | null;
}
export type EpisodeOrder = "aired" | "dvd" | "absolute" | "anime";
export interface EpisodeDetail {
  remote_id: string;
  season_number: number;
  episode_number: number;
  title: string;
  overview: string | null;
  order: EpisodeOrder;
}
export interface SeasonDetail {
  remote_id: string;
  season_number: number;
  title: string;
  episodes: EpisodeDetail[];
}
interface DetailData {
  identity: CandidateData;
  fields: RemoteField[];
  artwork: ArtworkCandidate[];
}
export interface MovieDetail extends DetailData {
  runtime_minutes: number | null;
}
export interface TvDetail extends DetailData {
  seasons: SeasonDetail[];
  order: EpisodeOrder;
}
export interface AnimeDetail extends DetailData {
  format: string | null;
  season: string | null;
  episodes: number | null;
  aliases: string[];
}
export type VideoMetadata =
  | { kind: "movie"; data: MovieDetail }
  | { kind: "tv"; data: TvDetail }
  | { kind: "anime"; data: AnimeDetail };
export interface MergeRequest {
  run_id: string;
  candidate_index: number;
  media_item_id: string;
  selected_fields: string[];
  selected_artwork: number[];
}
export interface FieldDiff {
  key: string;
  language: string | null;
  old: MetadataValue | null;
  new: RemoteField;
  provider_id: string;
  source_url: string;
  protected: boolean;
  needs_review: boolean;
  selected: boolean;
}
export interface MergePreview {
  schema_version: 1;
  preview_id: string;
  media_item_id: string;
  fields: FieldDiff[];
  artwork: ArtworkCandidate[];
  external_ids?: { namespace: string; old: string[]; new: string[] }[];
  confirmed: boolean;
}
export interface MergeResult {
  schema_version: 1;
  preview_id: string;
  applied_fields: number;
  already_applied: boolean;
}

function object(v: unknown): v is Record<string, unknown> {
  return v !== null && typeof v === "object" && !Array.isArray(v);
}
function text(v: unknown): v is string {
  return typeof v === "string" && v.length <= 2097152;
}
function nullableText(v: unknown): v is string | null {
  return v === null || text(v);
}
function integer(v: unknown): v is number {
  return typeof v === "number" && Number.isSafeInteger(v) && v >= 0;
}
function nullableInteger(v: unknown): v is number | null {
  return v === null || integer(v);
}
function list<T>(v: unknown, guard: (v: unknown) => v is T): v is T[] {
  return Array.isArray(v) && v.length <= 2000 && v.every(guard);
}
function url(v: unknown): v is string {
  if (!text(v)) return false;
  try {
    const u = new URL(v);
    return u.protocol === "https:" && !u.username && !u.password;
  } catch {
    return false;
  }
}
function id(v: unknown): v is RemoteId {
  return object(v) && text(v.namespace) && text(v.value);
}
function identity(v: unknown): v is CandidateData {
  return (
    object(v) &&
    [v.provider_id, v.remote_id, v.title, v.language].every(text) &&
    nullableText(v.original_title) &&
    nullableInteger(v.year) &&
    list(v.aliases, text) &&
    list(v.external_ids, id) &&
    url(v.source_url)
  );
}
function candidate(v: unknown): v is VideoCandidate {
  return (
    object(v) &&
    ["movie", "tv", "anime"].includes(String(v.kind)) &&
    identity(v.data)
  );
}
function remoteField(v: unknown): v is RemoteField {
  return object(v) && text(v.key) && "value" in v && nullableText(v.language);
}
function artwork(v: unknown): v is ArtworkCandidate {
  return (
    object(v) &&
    url(v.url) &&
    text(v.source_provider) &&
    text(v.kind) &&
    nullableText(v.language)
  );
}
function order(v: unknown): v is EpisodeOrder {
  return ["aired", "dvd", "absolute", "anime"].includes(String(v));
}
function episode(v: unknown): v is EpisodeDetail {
  return (
    object(v) &&
    text(v.remote_id) &&
    text(v.title) &&
    integer(v.season_number) &&
    integer(v.episode_number) &&
    nullableText(v.overview) &&
    order(v.order)
  );
}
function season(v: unknown): v is SeasonDetail {
  return (
    object(v) &&
    text(v.remote_id) &&
    text(v.title) &&
    integer(v.season_number) &&
    list(v.episodes, episode)
  );
}
export function searchPreview(v: unknown): v is SearchPreview {
  return (
    object(v) &&
    v.schema_version === 1 &&
    text(v.run_id) &&
    list(
      v.candidates,
      (c): c is MatchedCandidate =>
        object(c) &&
        candidate(c.candidate) &&
        list(c.reasons, text) &&
        typeof c.needs_review === "boolean" &&
        nullableInteger(c.duplicate_of),
    ) &&
    list(
      v.failures,
      (f): f is SearchPreview["failures"][number] =>
        object(f) &&
        text(f.provider_id) &&
        providerFailures.includes(f.failure as ProviderFailure),
    )
  );
}
export function videoMetadata(v: unknown): v is VideoMetadata {
  if (!object(v) || !object(v.data)) return false;
  const d = v.data;
  if (
    !identity(d.identity) ||
    !list(d.fields, remoteField) ||
    !list(d.artwork, artwork)
  )
    return false;
  switch (v.kind) {
    case "movie":
      return nullableInteger(d.runtime_minutes);
    case "tv":
      return order(d.order) && list(d.seasons, season);
    case "anime":
      return (
        nullableText(d.format) &&
        nullableText(d.season) &&
        nullableInteger(d.episodes) &&
        list(d.aliases, text)
      );
    default:
      return false;
  }
}
function metadataValue(v: unknown): v is MetadataValue {
  return (
    object(v) &&
    text(v.id) &&
    text(v.field_key) &&
    nullableText(v.locale) &&
    "value" in v &&
    nullableText(v.provider_id) &&
    nullableText(v.source_url) &&
    typeof v.is_user_modified === "boolean" &&
    typeof v.is_locked === "boolean"
  );
}
export function mergePreview(v: unknown): v is MergePreview {
  return (
    object(v) &&
    v.schema_version === 1 &&
    text(v.preview_id) &&
    text(v.media_item_id) &&
    list(
      v.fields,
      (f): f is FieldDiff =>
        object(f) &&
        text(f.key) &&
        nullableText(f.language) &&
        (f.old === null || metadataValue(f.old)) &&
        remoteField(f.new) &&
        text(f.provider_id) &&
        url(f.source_url) &&
        [f.protected, f.needs_review, f.selected].every(
          (x) => typeof x === "boolean",
        ),
    ) &&
    list(v.artwork, artwork) &&
    (v.external_ids === undefined ||
      list(v.external_ids, (entry): entry is NonNullable<MergePreview["external_ids"]>[number] =>
        object(entry) &&
        typeof entry.namespace === "string" && entry.namespace.length > 0 && entry.namespace.length <= 32 &&
        !/[\u0000-\u0020\u007f]/.test(entry.namespace) &&
        list(entry.old, (value): value is string => text(value) && value.length > 0 && value.length <= 256 && !/[\u0000-\u001f\u007f]/.test(value)) &&
        list(entry.new, (value): value is string => text(value) && value.length > 0 && value.length <= 256 && !/[\u0000-\u001f\u007f]/.test(value)),
      )) &&
    typeof v.confirmed === "boolean"
  );
}
function mergeResult(v: unknown): v is MergeResult {
  return (
    object(v) &&
    v.schema_version === 1 &&
    text(v.preview_id) &&
    integer(v.applied_fields) &&
    typeof v.already_applied === "boolean"
  );
}
export const scraperApi = {
  mediaDetail: (mediaId: string) =>
    request(
      "scraper_media_detail",
      { media_id: mediaId },
      (v): v is VideoMetadata | null => v === null || videoMetadata(v),
    ),
  searchPreview: (runId: string) =>
    request("scraper_search_preview", { run_id: runId }, searchPreview),
  search: (value: SearchRequest) =>
    request("scraper_search", value, searchPreview),
  detail: (runId: string, candidateIndex: number) =>
    request(
      "scraper_detail",
      { run_id: runId, candidate_index: candidateIndex },
      videoMetadata,
    ),
  preview: (value: MergeRequest) =>
    request("scraper_preview", value, mergePreview),
  artworkPreview: (
    runId: string,
    candidateIndex: number,
    artworkIndex: number,
  ) =>
    request(
      "scraper_artwork_preview",
      {
        run_id: runId,
        candidate_index: candidateIndex,
        artwork_index: artworkIndex,
      },
      (v): v is { thumbnail: string } =>
        object(v) &&
        typeof v.thumbnail === "string" &&
        v.thumbnail.length <= 2097152 &&
        /^data:image\/(png|jpeg|webp);base64,[A-Za-z0-9+/]+=*$/.test(
          v.thumbnail,
        ),
    ),
  confirm: (previewId: string) =>
    request("scraper_confirm", { preview_id: previewId }, mergeResult),
};
export interface ScraperHistoryEntry {
  id: string;
  query: VideoQuery;
  state: string;
  created_at: string;
  candidate_count: number;
}
export interface OutputFile {
  destination: string;
  kind: "nfo" | "poster" | "fanart" | "actor_thumb";
  old_text: string | null;
  new_text: string | null;
  expected_hash: string | null;
  content_hash: string;
  protected: boolean;
}
export interface OutputPreview {
  schema_version: 1;
  preview_id: string;
  media_item_id: string;
  strategy: "preserve" | "update_nfo";
  files: OutputFile[];
  state: string;
}
export interface OutputResult {
  schema_version: 1;
  preview_id: string;
  written: number;
  preserved: number;
  already_applied: boolean;
  state: string;
}
export type NfoExportMode = "online" | "offline";
function outputPreview(v: unknown): v is OutputPreview {
  return (
    object(v) &&
    v.schema_version === 1 &&
    text(v.preview_id) &&
    text(v.media_item_id) &&
    (v.strategy === "preserve" || v.strategy === "update_nfo") &&
    text(v.state) &&
    list(
      v.files,
      (f): f is OutputFile =>
        object(f) &&
        text(f.destination) &&
        ["nfo", "poster", "fanart", "actor_thumb"].includes(String(f.kind)) &&
        nullableText(f.old_text) &&
        nullableText(f.new_text) &&
        nullableText(f.expected_hash) &&
        text(f.content_hash) &&
        typeof f.protected === "boolean",
    )
  );
}
function outputResult(v: unknown): v is OutputResult {
  return (
    object(v) &&
    v.schema_version === 1 &&
    text(v.preview_id) &&
    integer(v.written) &&
    integer(v.preserved) &&
    typeof v.already_applied === "boolean" &&
    text(v.state)
  );
}
function history(v: unknown): v is ScraperHistoryEntry[] {
  return list(
    v,
    (h): h is ScraperHistoryEntry =>
      object(h) &&
      text(h.id) &&
      object(h.query) &&
      ["movie", "tv", "anime"].includes(String(h.query.kind)) &&
      text(h.query.title) &&
      nullableInteger(h.query.year) &&
      list(h.query.external_ids, id) &&
      text(h.query.language) &&
      text(h.state) &&
      text(h.created_at) &&
      Number.isFinite(Date.parse(h.created_at)) &&
      integer(h.candidate_count),
  );
}
export const scraperOutputApi = {
  defaultMode: (mediaId: string) =>
    request(
      "scraper_output_default_mode",
      { media_id: mediaId },
      (value): value is NfoExportMode => value === "online" || value === "offline",
    ),
  discard: (previewId: string) =>
    request(
      "scraper_output_discard",
      { preview_id: previewId },
      (v): v is null => v === null,
    ),
  preview: (mediaId: string, mode?: NfoExportMode, expectedDefaultMode?: NfoExportMode) =>
    request(
      "scraper_output_preview",
      { media_id: mediaId, ...(mode ? { mode } : {}), ...(expectedDefaultMode ? { expected_default_mode: expectedDefaultMode } : {}) },
      outputPreview,
    ),
  execute: (previewId: string) =>
    request("scraper_output_execute", { preview_id: previewId }, outputResult),
  history: () => request("scraper_history", {}, history),
};
