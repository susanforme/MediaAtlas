import type { BrowseQuery } from "../../api/library";
import { defaultBrowse } from "../../api/library";

export const TOOLS_KINDS = ["movie", "tv_series", "anime"] as const;
export const TOOLS_HEALTH = [
  "missing_poster",
  "missing_backdrop",
  "missing_nfo",
  "incomplete_metadata",
  "duplicate",
] as const;
export const TOOLS_SORTS = [
  "title_asc",
  "year_desc",
  "added_desc",
  "updated_desc",
] as const;
export const TOOLS_PAGE_SIZE = 24;
export interface ToolsSearch {
  media?: string;
  q?: string;
  root?: string;
  kind?: (typeof TOOLS_KINDS)[number];
  health?: (typeof TOOLS_HEALTH)[number];
  sort?: (typeof TOOLS_SORTS)[number];
  cursor?: string;
  page?: number;
}
export type ToolsFilters = Pick<
  ToolsSearch,
  "q" | "root" | "kind" | "health" | "sort"
>;
function boundedString(value: unknown, max: number): value is string {
  return typeof value === "string" && value.length > 0 && value.length <= max;
}
export function validateToolsSearch(
  value: Record<string, unknown>,
): ToolsSearch {
  const out: ToolsSearch = {};
  if (boundedString(value.media, 128)) out.media = value.media;
  if (boundedString(value.root, 128)) out.root = value.root;
  if (boundedString(value.q, 256)) out.q = value.q;
  if (TOOLS_KINDS.includes(value.kind as ToolsSearch["kind"] & string))
    out.kind = value.kind as ToolsSearch["kind"];
  if (TOOLS_HEALTH.includes(value.health as ToolsSearch["health"] & string))
    out.health = value.health as ToolsSearch["health"];
  if (TOOLS_SORTS.includes(value.sort as ToolsSearch["sort"] & string))
    out.sort = value.sort as ToolsSearch["sort"];
  if (boundedString(value.cursor, 2048)) out.cursor = value.cursor;
  const page =
    typeof value.page === "number" || typeof value.page === "string"
      ? Number(value.page)
      : NaN;
  if (out.cursor && Number.isSafeInteger(page) && page > 1) out.page = page;
  return out;
}
/** Changing a server filter invalidates the cursor, current detail and batch selection. */
export function changeToolsFilters(
  search: ToolsSearch,
  patch: ToolsFilters,
): ToolsSearch {
  return validateToolsSearch({
    ...search,
    ...patch,
    media: undefined,
    cursor: undefined,
    page: undefined,
  });
}
export function toolsFilterKey(search: ToolsSearch): string {
  return JSON.stringify([
    search.q ?? "",
    search.root ?? "",
    search.kind ?? "",
    search.health ?? "",
    search.sort ?? "updated_desc",
  ]);
}
export function toolsBrowseQuery(search: ToolsSearch): BrowseQuery {
  return {
    ...defaultBrowse(false),
    root_id: search.root ?? null,
    kind: search.kind ?? null,
    health: search.health ?? null,
    query: search.q ?? null,
    sort: search.sort ?? "updated_desc",
    cursor: search.cursor ?? null,
    limit: TOOLS_PAGE_SIZE,
  };
}
