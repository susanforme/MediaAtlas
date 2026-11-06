import { expect, it } from "vitest";
import type { NfoDocument } from "../../api/nfo";
import { nfoDetail } from "./detail-model";

function document(fields: Record<string, unknown>): NfoDocument {
  return { name: "sample.nfo", format: "xml", source: "<movie/>", poster_uri: null, backdrop_uri: null, metadata: { fields, external_ids: [] } };
}

it("uses only available NFO facts without inventing a media identity or missing values", () => {
  expect(nfoDetail(document({ title: " Atlas ", overview: "", outline: "Outline", studios: [], directors: null }))).toMatchObject({
    title: "Atlas", overview: "Outline", originalTitle: "", year: null, runtime: null, rating: null, genres: [], facts: [], cast: [],
  });
  expect(nfoDetail(document({})).title).toBe("sample.nfo");
});

it("keeps structured facts for shared presentation without JSON or duplicate people", () => {
  const detail = nfoDetail(document({
    cast: [{ name: "Actor", role: "Lead", thumb: "https://example.test/actor.jpg" }],
    people: ["Actor"],
    genres: ["Drama", " Drama ", null],
    rating: 0,
    ratings: [{ name: "tmdb", value: 0, max: 10, votes: 2 }],
    nfo_artwork: [{ kind: "poster", url: "https://example.test/poster.jpg" }],
  }));
  expect(detail.genres).toEqual(["Drama"]);
  expect(detail.rating).toEqual({ value: 0, provider: null });
  expect(detail.cast).toEqual(["Actor (Lead)"]);
  expect(detail.fields.find((field) => field.key === "ratings")?.value).toEqual([{ name: "tmdb", value: 0, max: 10, votes: 2 }]);
  expect(detail.fields.some((field) => ["people", "nfo_artwork"].includes(field.key))).toBe(false);
  expect(nfoDetail(document({ payload: { secret: "not a display field" } })).fields).toEqual([]);
});
