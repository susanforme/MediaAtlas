import {
  type BrowseQuery,
  type MediaDetail,
  type MediaPage,
  mediaCardGuard,
  mediaDetailGuard,
  type RootView,
  request,
  rootViewGuard,
  type ScanTask,
  scanTaskGuard,
} from "./library";
import {
  type ProviderFailure,
  providerFailures,
  providerSnapshot,
} from "./providers";
import {
  type ArtworkCandidate,
  type FieldDiff,
  type MergePreview,
  type MergeResult,
  mergePreview,
  type OutputPreview,
  type OutputResult,
} from "./scraper";

export { type AdultSettings, adultSettings } from "./adult-privacy";

import { adultPrivacyApi } from "./adult-privacy";
export interface CatalogNumber {
  raw: string;
  normalized: string;
  namespace: string;
  is_primary: boolean;
  aliases: string[];
  part: { kind: "cd" | "part"; number: number } | null;
}
export interface AdultQuery {
  catalog_number: string;
  namespace: string;
  studio: string | null;
  language: string;
}
export interface AdultCandidate {
  provider_id: string;
  remote_id: string;
  catalog_number: CatalogNumber;
  title: string;
  studio: string | null;
  source_url: string;
  language: string;
}
export interface AdultPerson {
  provider_id: string;
  remote_id: string;
  name: string;
  aliases: string[];
}
export interface AdultDetail {
  identity: AdultCandidate;
  label: string | null;
  series: string | null;
  release_date: string | null;
  runtime_minutes: number | null;
  performers: AdultPerson[];
  directors: AdultPerson[];
  artwork: ArtworkCandidate[];
}
export interface AdultSearchPreview {
  schema_version: 1;
  run_id: string;
  candidates: {
    candidate: AdultCandidate;
    reasons: string[];
    needs_review: boolean;
    duplicate_of: number | null;
  }[];
  failures: { provider_id: string; failure: ProviderFailure }[];
}
export interface AdultMergeRequest {
  run_id: string;
  candidate_index: number;
  media_item_id: string;
  selected_fields: string[];
  selected_artwork: number[];
}
export type AdultMergePreview = MergePreview;
export type AdultFieldDiff = FieldDiff;
export interface AdultOutputFile
  extends Omit<OutputPreview["files"][number], "kind"> {
  kind: "nfo" | "poster" | "cover" | "back_cover" | "sample";
}
export interface AdultOutputPreview extends Omit<OutputPreview, "files"> {
  files: AdultOutputFile[];
}
export interface AdultBatchPreview {
  schema_version: 1;
  batch_id: string;
  state: string;
  items: AdultOutputPreview[];
}
export interface AdultBatchResult {
  schema_version: 1;
  batch_id: string;
  state: string;
  items: {
    media_item_id: string;
    preview_id: string;
    state: string;
    result: OutputResult | null;
    error_code: string | null;
  }[];
}
const object = (v: unknown): v is Record<string, unknown> =>
  v !== null && typeof v === "object" && !Array.isArray(v);
const text = (v: unknown): v is string =>
  typeof v === "string" && v.length <= 2097152;
const maybeText = (v: unknown): v is string | null => v === null || text(v);
const integer = (v: unknown): v is number =>
  typeof v === "number" && Number.isSafeInteger(v) && v >= 0;
const maybeInteger = (v: unknown): v is number | null =>
  v === null || integer(v);
const list = <T>(v: unknown, guard: (x: unknown) => x is T): v is T[] =>
  Array.isArray(v) && v.length <= 2000 && v.every(guard);
const url = (v: unknown): v is string => {
  if (!text(v)) return false;
  try {
    const u = new URL(v);
    return u.protocol === "https:" && !u.username && !u.password;
  } catch {
    return false;
  }
};
export function catalogNumber(v: unknown): v is CatalogNumber {
  return (
    object(v) &&
    [v.raw, v.normalized, v.namespace].every(text) &&
    typeof v.is_primary === "boolean" &&
    list(v.aliases, text) &&
    (v.part === null ||
      (object(v.part) &&
        ["cd", "part"].includes(String(v.part.kind)) &&
        integer(v.part.number) &&
        v.part.number > 0))
  );
}
function candidate(v: unknown): v is AdultCandidate {
  return (
    object(v) &&
    [v.provider_id, v.remote_id, v.title, v.language].every(text) &&
    catalogNumber(v.catalog_number) &&
    maybeText(v.studio) &&
    url(v.source_url)
  );
}
function person(v: unknown): v is AdultPerson {
  return (
    object(v) &&
    [v.provider_id, v.remote_id, v.name].every(text) &&
    list(v.aliases, text)
  );
}
function artwork(v: unknown): v is ArtworkCandidate {
  return (
    object(v) &&
    url(v.url) &&
    text(v.source_provider) &&
    text(v.kind) &&
    maybeText(v.language)
  );
}
export function adultDetail(v: unknown): v is AdultDetail {
  return (
    object(v) &&
    candidate(v.identity) &&
    [v.label, v.series, v.release_date].every(maybeText) &&
    maybeInteger(v.runtime_minutes) &&
    list(v.performers, person) &&
    list(v.directors, person) &&
    list(v.artwork, artwork)
  );
}
export function adultSearchPreview(v: unknown): v is AdultSearchPreview {
  return (
    object(v) &&
    v.schema_version === 1 &&
    text(v.run_id) &&
    list(
      v.candidates,
      (c): c is AdultSearchPreview["candidates"][number] =>
        object(c) &&
        candidate(c.candidate) &&
        list(c.reasons, text) &&
        typeof c.needs_review === "boolean" &&
        maybeInteger(c.duplicate_of),
    ) &&
    list(
      v.failures,
      (f): f is AdultSearchPreview["failures"][number] =>
        object(f) &&
        text(f.provider_id) &&
        providerFailures.includes(f.failure as ProviderFailure),
    )
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
export function adultOutputPreview(v: unknown): v is AdultOutputPreview {
  return (
    object(v) &&
    v.schema_version === 1 &&
    text(v.preview_id) &&
    text(v.media_item_id) &&
    v.strategy === "preserve" &&
    text(v.state) &&
    list(
      v.files,
      (f): f is AdultOutputFile =>
        object(f) &&
        text(f.destination) &&
        ["nfo", "poster", "cover", "back_cover", "sample"].includes(
          String(f.kind),
        ) &&
        maybeText(f.old_text) &&
        maybeText(f.new_text) &&
        maybeText(f.expected_hash) &&
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
export function adultBatchPreview(v: unknown): v is AdultBatchPreview {
  return (
    object(v) &&
    v.schema_version === 1 &&
    text(v.batch_id) &&
    text(v.state) &&
    list(v.items, adultOutputPreview) &&
    v.items.length <= 32
  );
}
export function adultBatchResult(v: unknown): v is AdultBatchResult {
  return (
    object(v) &&
    v.schema_version === 1 &&
    text(v.batch_id) &&
    text(v.state) &&
    list(
      v.items,
      (i): i is AdultBatchResult["items"][number] =>
        object(i) &&
        [i.media_item_id, i.preview_id, i.state].every(text) &&
        (i.result === null || outputResult(i.result)) &&
        maybeText(i.error_code),
    )
  );
}
// Local library view guards validate adult identity again before display. The backend owns paths and visibility.
function mediaPage(v: unknown): v is MediaPage {
  return (
    object(v) &&
    list(
      v.items,
      (item): item is MediaPage["items"][number] =>
        mediaCardGuard(item) && item.media.kind === "adult_video",
    ) &&
    maybeText(v.next_cursor)
  );
}
function mediaDetail(v: unknown): v is MediaDetail {
  return mediaDetailGuard(v) && v.media.kind === "adult_video";
}
export const adultApi = {
  toolsMedia: (value: BrowseQuery) =>
    request("adult_tools_media", value, mediaPage),
  toolsDetail: (mediaId: string) =>
    request("adult_tools_detail", { media_id: mediaId }, mediaDetail),
  tasks: () =>
    request("adult_tasks", {}, (v): v is ScanTask[] => list(v, scanTaskGuard)),
  recent: () =>
    request(
      "adult_recent",
      {},
      (v): v is { media_id: string; title: string; visited_at: string }[] =>
        list(
          v,
          (r): r is { media_id: string; title: string; visited_at: string } =>
            object(r) &&
            text(r.media_id) &&
            text(r.title) &&
            text(r.visited_at) &&
            Number.isFinite(Date.parse(r.visited_at)),
        ),
    ),
  ...adultPrivacyApi,
  media: (value: BrowseQuery) => request("adult_list_media", value, mediaPage),
  getMedia: (mediaId: string) =>
    request("adult_get_media", { media_id: mediaId }, mediaDetail),
  roots: () =>
    request("adult_list_roots", {}, (v): v is RootView[] =>
      list(
        v,
        (root): root is RootView =>
          rootViewGuard(root) && root.kind === "adult_video",
      ),
    ),
  providers: () => request("adult_providers", {}, providerSnapshot),
  search: (query: AdultQuery, providers: string[]) =>
    request("adult_search", { query, providers }, adultSearchPreview),
  detail: (runId: string, candidateIndex: number) =>
    request(
      "adult_detail",
      { run_id: runId, candidate_index: candidateIndex },
      adultDetail,
    ),
  preview: (value: AdultMergeRequest) =>
    request("adult_preview", value, mergePreview),
  confirm: (previewId: string) =>
    request("adult_confirm", { preview_id: previewId }, mergeResult),
  mediaDetail: (mediaId: string) =>
    request(
      "adult_media_detail",
      { media_id: mediaId },
      (v): v is AdultDetail | null => v === null || adultDetail(v),
    ),
  artworkPreview: (
    runId: string,
    candidateIndex: number,
    artworkIndex: number,
  ) =>
    request(
      "adult_artwork_preview",
      {
        run_id: runId,
        candidate_index: candidateIndex,
        artwork_index: artworkIndex,
      },
      (v): v is { thumbnail: string } =>
        object(v) &&
        text(v.thumbnail) &&
        /^data:image\/(png|jpeg|webp);base64,[A-Za-z0-9+/]+=*$/.test(
          v.thumbnail,
        ),
    ),
  outputPreview: (mediaId: string) =>
    request("adult_output_preview", { media_id: mediaId }, adultOutputPreview),
  outputExecute: (previewId: string) =>
    request("adult_output_execute", { preview_id: previewId }, outputResult),
  outputDiscard: (previewId: string) =>
    request(
      "adult_output_discard",
      { preview_id: previewId },
      (v): v is null => v === null,
    ),
  batchPreview: (mediaIds: string[]) =>
    request("adult_batch_preview", { media_ids: mediaIds }, adultBatchPreview),
  batchExecute: (batchId: string) =>
    request("adult_batch_execute", { batch_id: batchId }, adultBatchResult),
  batchCancel: (batchId: string) =>
    request("adult_batch_cancel", { batch_id: batchId }, adultBatchResult),
};
