import { request } from "./library";

export type ReviewState = "pending" | "deferred" | "confirmed" | "failed";
export interface ReviewContext {
  root?: string;
  kind?: "movie" | "tv_series" | "anime";
  health?: string;
  q?: string;
  sort?: "title_asc" | "year_desc" | "added_desc" | "updated_desc";
  cursor?: string;
  page?: number;
}
export interface ReviewItem {
  media_id: string;
  position: number;
  state: ReviewState;
  checkpoint: string | null;
  preview_id: string | null;
  needs_recheck: boolean;
  reason:
    | "missing_media"
    | "unavailable_media"
    | "stale_checkpoint"
    | "expired_item"
    | null;
}
export interface ReviewQueue {
  id: string;
  revision: number;
  current_media_id: string | null;
  context: ReviewContext;
  items: ReviewItem[];
  created_at: string;
  updated_at: string;
}
export interface ReviewUpdate {
  queue_id: string;
  media_id: string;
  expected_revision: number;
  action: "select" | "defer" | "retry" | "failed" | "confirmed";
  checkpoint?: string;
}
const uuid = (value: unknown): value is string =>
  typeof value === "string" &&
  /^[\da-f]{8}-[\da-f]{4}-[\da-f]{4}-[\da-f]{4}-[\da-f]{12}$/i.test(value);
const record = (value: unknown): value is Record<string, unknown> =>
  value !== null && typeof value === "object" && !Array.isArray(value);
const uint = (value: unknown): value is number =>
  Number.isSafeInteger(value) && Number(value) >= 0;
const nullableUuid = (value: unknown) => value === null || uuid(value);
const safeText = (value: unknown, max: number) =>
  typeof value === "string" &&
  value.length > 0 &&
  value.length <= max &&
  [...value].every(
    (character) =>
      character.charCodeAt(0) >= 32 && character.charCodeAt(0) !== 127,
  );
const health = [
  "missing_poster",
  "missing_backdrop",
  "missing_nfo",
  "incomplete_metadata",
  "duplicate",
  "metadata_pending",
  "metadata_needs_review",
  "metadata_failed",
  "metadata_unconfigured",
];
function context(value: unknown): value is ReviewContext {
  if (!record(value)) return false;
  return Object.entries(value).every(([key, field]) => {
    if (key === "root") return uuid(field);
    if (key === "kind")
      return ["movie", "tv_series", "anime"].includes(String(field));
    if (key === "health") return health.includes(String(field));
    if (key === "sort")
      return ["title_asc", "year_desc", "added_desc", "updated_desc"].includes(
        String(field),
      );
    if (key === "q") return safeText(field, 256);
    if (key === "cursor") return safeText(field, 2048);
    if (key === "page") return uint(field) && field >= 1 && field <= 1_000_000;
    return false;
  });
}
function item(value: unknown): value is ReviewItem {
  return (
    record(value) &&
    uuid(value.media_id) &&
    uint(value.position) &&
    value.position < 32 &&
    ["pending", "deferred", "confirmed", "failed"].includes(
      String(value.state),
    ) &&
    nullableUuid(value.checkpoint) &&
    nullableUuid(value.preview_id) &&
    typeof value.needs_recheck === "boolean" &&
    [
      null,
      "missing_media",
      "unavailable_media",
      "stale_checkpoint",
      "expired_item",
    ].includes(value.reason as string | null)
  );
}
export function isReviewQueue(value: unknown): value is ReviewQueue {
  return (
    record(value) &&
    uuid(value.id) &&
    uint(value.revision) &&
    nullableUuid(value.current_media_id) &&
    context(value.context) &&
    Array.isArray(value.items) &&
    value.items.length > 0 &&
    value.items.length <= 32 &&
    value.items.every(item) &&
    new Set(value.items.map((entry) => entry.media_id)).size ===
      value.items.length &&
    value.items.every((entry, index) => entry.position === index) &&
    (value.current_media_id === null ||
      value.items.some((entry) => entry.media_id === value.current_media_id)) &&
    safeText(value.created_at, 64) &&
    safeText(value.updated_at, 64)
  );
}
const forQueue =
  (id: string) =>
  (value: unknown): value is ReviewQueue =>
    isReviewQueue(value) && value.id === id;
export const reviewQueueApi = {
  create: (mediaIds: string[], returnContext: ReviewContext) =>
    request(
      "review_queue_create",
      { media_ids: mediaIds, context: returnContext },
      (value): value is ReviewQueue =>
        isReviewQueue(value) &&
        value.items.length === mediaIds.length &&
        value.items.every((item, index) => item.media_id === mediaIds[index]),
    ),
  list: () =>
    request(
      "review_queue_list",
      {},
      (value): value is ReviewQueue[] =>
        Array.isArray(value) &&
        value.length <= 20 &&
        value.every(isReviewQueue),
    ),
  get: (queueId: string) =>
    request("review_queue_get", { queue_id: queueId }, forQueue(queueId)),
  update: (input: ReviewUpdate) =>
    request("review_queue_update", input, forQueue(input.queue_id)),
  checkpoint: (input: {
    queue_id: string;
    media_id: string;
    expected_revision: number;
    preview_id: string;
  }) =>
    request("review_queue_checkpoint", input, (value): value is ReviewQueue => {
      if (
        !forQueue(input.queue_id)(value) ||
        value.current_media_id !== input.media_id ||
        value.revision <= input.expected_revision
      )
        return false;
      const current = value.items.find(
        (item) => item.media_id === input.media_id,
      );
      return Boolean(
        current &&
          current.state === "pending" &&
          !current.needs_recheck &&
          current.checkpoint &&
          current.preview_id === input.preview_id,
      );
    }),
};
