import { expect, it } from "vitest";
import {
  changeToolsFilters,
  toolsBrowseQuery,
  validateToolsSearch,
} from "./search";

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
