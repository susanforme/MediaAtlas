import { expect, it } from "vitest";
import {
  changeToolsFilters,
  changeToolsRouteFilters,
  toolsBrowseQuery,
  toolsFilterKey,
  toolsRouteBrowseQuery,
  toolsSectionLocation,
  validateToolsRouteSearch,
  validateToolsSearch,
} from "./search";

it("keys workspace lifetime by filters while retaining it across pagination and detail navigation", () => {
  const base = validateToolsRouteSearch({ kind: "movie", root: "folder", q: "Atlas" });
  const nextPage = validateToolsRouteSearch({ ...base, cursor: "next", page: 2, media: "film", view: "health" });
  expect(toolsFilterKey(nextPage)).toBe(toolsFilterKey(base));
  expect(toolsFilterKey({ ...base, sort: "updated_desc" })).toBe(toolsFilterKey(base));
  expect(toolsFilterKey({ ...base, q: "Another" })).not.toBe(toolsFilterKey(base));
  expect(toolsFilterKey({ ...base, kind: "adult_video" })).not.toBe(toolsFilterKey(base));
});

it("routes the adult type separately without granting ordinary query visibility", () => {
  expect(
    validateToolsRouteSearch({
      kind: "adult_video",
      media: "private-id",
      q: "private",
      include_adult: true,
    }),
  ).toEqual({ kind: "adult_video", media: "private-id", q: "private" });
  expect(
    validateToolsSearch({ kind: "adult_video", include_adult: true }),
  ).toEqual({});
  expect(
    toolsBrowseQuery(validateToolsSearch({ kind: "adult_video" }))
      .include_adult,
  ).toBe(false);
});

it("keeps the health view in route state without sending navigation fields to backend filters", () => {
  const route = validateToolsRouteSearch({ view: "health", q: "Atlas", root: "root", media: "film" });
  expect(route).toEqual({ view: "health", q: "Atlas", root: "root", media: "film" });
  expect(validateToolsSearch({ ...route })).toEqual({ q: "Atlas", root: "root", media: "film" });
  expect(toolsSectionLocation("local", route)).toEqual({ to: "/tools", search: { q: "Atlas", root: "root", media: "film" } });
  expect(toolsSectionLocation("scraper", route)).toEqual({ to: "/scraper", search: { media: "film" } });
  expect(changeToolsRouteFilters(route, { q: "New" })).toEqual({ view: "health", q: "New", root: "root" });
  expect(validateToolsRouteSearch({ view: "unknown" })).toEqual({});
  expect(validateToolsRouteSearch({ kind: "adult_video", view: "health", media: "private" })).toEqual({ kind: "adult_video", media: "private" });
  expect(toolsSectionLocation("scraper", { kind: "adult_video", media: "private" })).toEqual({ to: "/scraper", search: { media: undefined } });
});

it("accepts only bounded, whitelisted ordinary tool query fields", () => {
  expect(
    validateToolsSearch({
      media: "media-id",
      q: "search",
      root: "root-id",
      kind: "movie",
      health: "missing_nfo",
      sort: "year_desc",
      cursor: "cursor",
      page: "2",
      queue: ["not-a-backend-choice"],
      include_adult: true,
    }),
  ).toEqual({
    media: "media-id",
    q: "search",
    root: "root-id",
    kind: "movie",
    health: "missing_nfo",
    sort: "year_desc",
    cursor: "cursor",
    page: 2,
  });
  expect(
    validateToolsSearch({
      kind: "adult_video",
      health: "invented",
      sort: "random",
      root: "x".repeat(129),
      q: "x".repeat(257),
      cursor: "x".repeat(2049),
      page: 9,
    }),
  ).toEqual({});
  expect(validateToolsSearch({ cursor: "valid", page: [2] })).toEqual({
    cursor: "valid",
  });
});
it("resets invalid cursor, page and detail when changing a server filter while retaining other filters", () => {
  const next = changeToolsFilters(
    {
      media: "selected",
      q: "old",
      root: "root",
      kind: "anime",
      cursor: "old-page",
      page: 6,
      sort: "title_asc",
    },
    { q: "new" },
  );
  expect(next).toEqual({
    q: "new",
    root: "root",
    kind: "anime",
    sort: "title_asc",
  });
  expect(toolsBrowseQuery(next)).toMatchObject({
    query: "new",
    root_id: "root",
    kind: "anime",
    cursor: null,
    sort: "title_asc",
    include_adult: false,
  });
});

it("retains shared server filters for the adult adapter and clears cross-type root/detail selections", () => {
  const search = validateToolsRouteSearch({
    kind: "adult_video",
    root: "adult-root",
    q: "code",
    health: "missing_poster",
    sort: "title_asc",
    cursor: "next",
    page: 2,
  });
  expect(toolsRouteBrowseQuery(search)).toMatchObject({
    kind: "adult_video",
    include_adult: true,
    root_id: "adult-root",
    query: "code",
    cursor: "next",
    sort: "title_asc",
  });
  expect(
    changeToolsRouteFilters(
      { ...search, media: "selected" },
      { kind: "movie" },
    ),
  ).toEqual({
    kind: "movie",
    q: "code",
    health: "missing_poster",
    sort: "title_asc",
  });
  expect(
    toolsBrowseQuery(validateToolsSearch({ ...search })).include_adult,
  ).toBe(false);
});
