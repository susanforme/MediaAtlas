import { expect, it } from "vitest";
import type { ProviderView } from "../../api/providers";
import { directorySourceChoices, moveRootSource } from "./root-source-model";

it("reorders a directory's sources without changing another saved list", () => {
  const sources = ["tmdb", "tvmaze"];
  expect(moveRootSource(sources, 1, -1)).toEqual(["tvmaze", "tmdb"]);
  expect(sources).toEqual(["tmdb", "tvmaze"]);
  expect(moveRootSource(sources, 0, -1)).toBe(sources);
  expect(moveRootSource(sources, 1, 1)).toBe(sources);
  expect(moveRootSource([], 0, 1)).toEqual([]);
});

it("offers only compatible real automatic sources from the current channel", () => {
  const provider = (id: string, kinds: string[], fixture = false) => ({
    descriptor: { id, media_kinds: kinds, fixture },
  }) as ProviderView;
  const providers = [
    provider("tmdb", ["movie", "tv_series"]),
    provider("omdb", ["movie", "tv_series"]),
    provider("douban", ["movie", "tv_series"]),
    provider("tvmaze", ["tv_series"]),
    provider("anilist", ["anime"]),
    provider("bangumi", ["anime"]),
    provider("dmm", ["adult_video"]),
    provider("wikidata", ["adult_video"]),
    provider("wikipedia.html", ["movie"]),
    provider("tmdb", ["movie"], true),
  ];
  expect(directorySourceChoices(providers, "tv_series").map((p) => p.descriptor.id)).toEqual(["tmdb", "omdb", "douban", "tvmaze"]);
  expect(directorySourceChoices(providers, "anime").map((p) => p.descriptor.id)).toEqual(["anilist", "bangumi"]);
  expect(directorySourceChoices(providers, "adult_video").map((p) => p.descriptor.id)).toEqual(["dmm", "wikidata"]);
  expect(directorySourceChoices(providers, null).map((p) => p.descriptor.id)).toEqual(["tmdb", "omdb", "douban", "tvmaze", "anilist", "bangumi"]);
  expect(directorySourceChoices(providers, "music_release")).toEqual([]);
});
it("keeps optional movie and anime source priorities confined to the edited list", () => {
  const movies = ["tmdb", "omdb", "douban"];
  const anime = ["anilist", "bangumi"];
  expect(moveRootSource(movies, 2, -1)).toEqual(["tmdb", "douban", "omdb"]);
  expect(moveRootSource(anime, 1, -1)).toEqual(["bangumi", "anilist"]);
  expect(movies).toEqual(["tmdb", "omdb", "douban"]);
  expect(anime).toEqual(["anilist", "bangumi"]);
});
