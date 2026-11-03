import { expect, it } from "vitest";
import {
  changeToolsFilters,
  changeToolsRouteFilters,
  toolsBrowseQuery,
  toolsRouteBrowseQuery,
  validateToolsRouteSearch,
  validateToolsSearch,
} from "./search";

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
