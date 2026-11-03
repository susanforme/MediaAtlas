import { expect, it } from "vitest";
import { validateToolsSearch } from "../tools/search";
import { scrapeKind } from "./scraper-model";
import {
  MAX_SCRAPE_QUEUE,
  validateScraperReturnSearch,
  validateScraperSearch,
} from "./search";

it.each([validateScraperSearch, validateToolsSearch])(
  "accepts only a bounded media context and drops unrelated values",
  (validate) => {
    expect(validate({ media: "local-media", query: "ignored" })).toEqual({
      media: "local-media",
    });
    expect(validate({ media: "x".repeat(128) })).toEqual({
      media: "x".repeat(128),
    });
    for (const media of [
      undefined,
      null,
      "",
      "x".repeat(129),
      4,
      ["private"],
      { id: "private" },
    ])
      expect(validate({ media })).toEqual({});
  },
);
it("maps only explicitly supported ordinary media kinds", () => {
  expect(scrapeKind("movie")).toBe("movie");
  expect(scrapeKind("tv_series")).toBe("tv");
  expect(scrapeKind("anime")).toBe("anime");
  for (const kind of [
    "adult_video",
    "tv_episode",
    "tv_season",
    "music_release",
    "unknown",
  ])
    expect(scrapeKind(kind)).toBeNull();
});

it("preserves only the fixed scraper return context, never a return URL", () => {
  expect(
    validateScraperReturnSearch({
      from: "scraper",
      media: "local-media",
      returnUrl: "https://example.com",
    }),
  ).toEqual({ from: "scraper", media: "local-media" });
  expect(validateScraperReturnSearch({ from: "scraper" })).toEqual({
    from: "scraper",
  });
  expect(
    validateScraperReturnSearch({ from: "scraper", media: "x".repeat(129) }),
  ).toEqual({ from: "scraper" });
  for (const from of [
    undefined,
    null,
    "https://example.com",
    "/adult",
    ["scraper"],
    { from: "scraper" },
  ])
    expect(validateScraperReturnSearch({ from, media: "local-media" })).toEqual(
      {},
    );
});

it("normalizes a bounded queue in order and picks its first item when no current media exists", () => {
  expect(MAX_SCRAPE_QUEUE).toBe(32);
  expect(
    validateScraperSearch({ queue: ["second", "first", "second"] }),
  ).toEqual({ media: "second", queue: ["second", "first"] });
  expect(
    validateScraperSearch({ media: "first", queue: ["second", "first"] }),
  ).toEqual({ media: "first", queue: ["second", "first"] });
  const queue = Array.from(
    { length: MAX_SCRAPE_QUEUE },
    (_, index) => `media-${index}`,
  );
  expect(validateScraperSearch({ queue })).toEqual({ media: "media-0", queue });
});
it("rejects an entire malformed or oversized queue instead of silently processing a partial list", () => {
  const invalid = [
    "first",
    [],
    ["first", 3],
    ["first", ""],
    ["first", " "],
    ["first", "x".repeat(129)],
    ["first", "line\nbreak"],
    ["first", null],
    Array(MAX_SCRAPE_QUEUE + 1).fill("same-id"),
    new Array(2),
  ];
  for (const queue of invalid)
    expect(validateScraperSearch({ media: "current", queue })).toEqual({
      media: "current",
    });
  expect(
    validateScraperSearch({ media: "outside", queue: ["first", "second"] }),
  ).toEqual({ media: "outside" });
});
it("keeps only a valid queue when returning from the fixed scraper setup flow", () => {
  expect(
    validateScraperReturnSearch({
      from: "scraper",
      media: "second",
      queue: ["first", "second", "first"],
      returnUrl: "/adult",
    }),
  ).toEqual({ from: "scraper", media: "second", queue: ["first", "second"] });
  expect(
    validateScraperReturnSearch({ from: "external", queue: ["first"] }),
  ).toEqual({});
});
