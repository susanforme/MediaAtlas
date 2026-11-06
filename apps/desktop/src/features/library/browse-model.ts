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

export function hasLibraryFilters(search: LibrarySearch): boolean {
  return Boolean(
    search.q ||
      search.health ||
      search.genre ||
      search.person ||
      search.studio ||
      search.year,
  );
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

/** Automatic metadata task predicates are not part of the adult browse contract. */
export function supportedLibraryHealth(
  health: string | undefined,
  adult: boolean,
): string | undefined {
  if (!health || (adult && health.startsWith("metadata_"))) return undefined;
  return health;
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
    health: supportedLibraryHealth(search.health, adult) ?? null,
    genre: search.genre ?? null,
    person: search.person ?? null,
    studio: search.studio ?? null,
    year: search.year ?? null,
    sort: search.sort ?? "updated_desc",
    cursor: search.cursor ?? null,
  };
}
