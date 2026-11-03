import { expect, it } from "vitest";
import { validateToolsSearch } from "../tools/search";
import { scrapeKind } from "./scraper-model";
import { validateScraperSearch } from "./search";

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
