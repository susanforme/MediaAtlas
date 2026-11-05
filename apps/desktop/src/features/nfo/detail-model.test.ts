import { expect, it } from "vitest";
import type { NfoDocument } from "../../api/nfo";
import { nfoDetail } from "./detail-model";

function document(fields: Record<string, unknown>): NfoDocument {
  return { name: "sample.nfo", format: "xml", source: "<movie/>", metadata: { fields, external_ids: [] } };
}

it("uses only available NFO facts without inventing a media identity or missing values", () => {
  expect(nfoDetail(document({ title: " Atlas ", overview: "", outline: "Outline", studios: [], directors: null }))).toMatchObject({
    title: "Atlas", overview: "Outline", originalTitle: "", year: null, runtime: null, rating: null, genres: [], fields: [],
  });
  expect(nfoDetail(document({})).title).toBe("sample.nfo");
});

it("preserves cast roles, source ratings and inert artwork references without duplicate people", () => {
  const detail = nfoDetail(document({
    cast: [{ name: "Actor", role: "Lead", thumb: "https://example.test/actor.jpg" }],
    people: ["Actor"],
    genres: ["Drama", " Drama ", null],
    rating: 0,
    ratings: [{ name: "tmdb", value: 0, votes: 2 }],
    nfo_artwork: [{ kind: "poster", url: "https://example.test/poster.jpg" }],
  }));
  expect(detail.genres).toEqual(["Drama"]);
  expect(detail.rating).toEqual({ value: 0, provider: null });
  expect(detail.fields).toEqual([
    { key: "cast", value: "Actor (Lead)" },
    { key: "ratings", value: '{"name":"tmdb","value":0,"votes":2}' },
    { key: "nfo_artwork", value: '{"kind":"poster","url":"https://example.test/poster.jpg"}' },
  ]);
});
