import { expect, it } from "vitest";
import { castNames, hasMetadataValue, metadataRatings, metadataText } from "./media-metadata-model";

it("formats actual scalar lists and people without serializing unknown objects", () => {
  expect(metadataText(["Drama", null, "Thriller"])).toBe("Drama / Thriller");
  expect(metadataText(0)).toBe("0");
  expect(metadataText({ nested: "not a display field" })).toBe("");
  expect(castNames([{ name: "Actor", role: "Lead", thumb: "https://example.test/a.jpg" }, "Guest", null])).toEqual(["Actor (Lead)", "Guest"]);
  expect(hasMetadataValue("nfo_artwork", [{ kind: "poster", url: "file:///private" }])).toBe(false);
});

it("keeps real collection and rating shapes while excluding invalid scales", () => {
  expect(hasMetadataValue("collection", { name: "Collection", overview: "Story" })).toBe(true);
  expect(metadataRatings([{ name: "tmdb", value: 0, max: 10, votes: 0 }, { name: "bad", value: 11, max: 10 }])).toEqual([{ name: "tmdb", value: 0, max: 10, votes: 0 }]);
});
