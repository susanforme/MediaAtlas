import type { BrowseQuery, MediaKind } from "../../api/library";
import { defaultBrowse } from "../../api/library";
import { ALL_KINDS } from "./library-model";

export interface LibrarySearch {
  root?: string;
  kind?: MediaKind;
  q?: string;
  health?: string;
  genre?: string;
  person?: string;
  studio?: string;
  year?: number;
  sort?: string;
  cursor?: string;
  page?: number;
  view?: "wall" | "list";
}
export function validateLibrarySearch(
  value: Record<string, unknown>,
): LibrarySearch {
  const out: LibrarySearch = {};
  for (const key of [
    "root",
    "q",
    "health",
    "genre",
    "person",
    "studio",
    "sort",
    "cursor",
  ] as const)
    if (typeof value[key] === "string" && value[key])
      out[key] = value[key] as string;
  if (ALL_KINDS.includes(value.kind as MediaKind))
    out.kind = value.kind as MediaKind;
  const year = Number(value.year);
  if (value.year && Number.isInteger(year)) out.year = year;
  const page = Number(value.page);
  if (Number.isInteger(page) && page > 0) out.page = page;
  if (value.view === "list" || value.view === "wall") out.view = value.view;
  return out;
}

export function libraryBrowseQuery(
  search: LibrarySearch,
  adult: boolean,
): BrowseQuery {
  return {
    ...defaultBrowse(false),
    root_id: search.root ?? null,
    kind: adult
      ? "adult_video"
      : search.kind === "adult_video"
        ? null
        : (search.kind ?? null),
    query: search.q ?? null,
    health: search.health ?? null,
    genre: search.genre ?? null,
    person: search.person ?? null,
    studio: search.studio ?? null,
    year: search.year ?? null,
    sort: search.sort ?? "updated_desc",
    cursor: search.cursor ?? null,
  };
}
