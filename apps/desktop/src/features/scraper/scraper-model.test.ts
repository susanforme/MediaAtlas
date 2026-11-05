import { expect, it } from "vitest";
import { syntheticMerge, syntheticMetadata } from "./fixtures";
import { changedExternalIds, identityMayChange, value } from "./scraper-model";

it("shows actor names and roles without exposing image URLs or dropping legacy names", () => {
  expect(value([
    { name: "Actor A", role: "Detective", thumb: "https://image.tmdb.org/a.jpg", tmdb_id: "123" },
    { name: "Actor B", role: "" },
    "Legacy actor",
  ])).toBe("Actor A (Detective)、Actor B、Legacy actor");
  expect(value({ name: "Film collection", tmdb_id: "456" })).toBe("Film collection");
  expect(value(["Drama", "Mystery"])).toBe("Drama、Mystery");
});

it("uses new source identities only to request validation and displays only authoritative ID changes", () => {
  expect(identityMayChange(syntheticMetadata)).toBe(true);
  const equal = { ...syntheticMerge, external_ids: [
    { namespace: "imdb", old: ["tt1"], new: ["tt1"] },
    { namespace: "tmdb", old: ["old"], new: ["new"] },
    { namespace: "douban", old: ["1"], new: [] },
    { namespace: "bangumi", old: [], new: ["2"] },
  ] };
  expect(changedExternalIds(equal).map((entry) => entry.namespace)).toEqual(["tmdb", "douban", "bangumi"]);
  expect(changedExternalIds(syntheticMerge)).toEqual([]);
  expect(changedExternalIds({ ...syntheticMerge, external_ids: [
    { namespace: "local", old: ["1", "2"], new: ["2", "1"] },
  ] })).toEqual([]);
});
