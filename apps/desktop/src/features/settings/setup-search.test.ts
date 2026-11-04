import { expect, it } from "vitest";
import { setupReturnLocation, validateProviderSetupSearch } from "./setup-search";

it("preserves only supported library setup context and settings tabs", () => {
  expect(
    validateProviderSetupSearch({
      from: "library",
      root: "folder-1",
      kind: "anime",
      provider: "anilist",
      tab: "auth",
      return: "https://outside.test",
      media: "unrelated",
    }),
  ).toEqual({
    from: "library",
    root: "folder-1",
    kind: "anime",
    provider: "anilist",
    tab: "auth",
  });
  expect(
    validateProviderSetupSearch({
      from: "library",
      root: "../secret",
      kind: "invented",
      provider: "unknown",
      tab: "unknown",
    }),
  ).toEqual({ from: "library" });
});

it("preserves library filters and pagination through setup using a fixed destination", () => {
  const search = {
    root: "folder-1",
    kind: "movie" as const,
    q: "Solaris",
    health: "metadata_unconfigured",
    sort: "title_asc",
    page: 3,
    cursor: "next-page",
    view: "list" as const,
  };
  const setup = validateProviderSetupSearch({
    from: "library",
    root: "folder-1",
    kind: "movie",
    return: { to: "library", search: { ...search, extra: "ignored" } },
  });
  expect(setup.return).toEqual({ to: "library", search });
  expect(setupReturnLocation(setup)).toEqual({ to: "/library", search });
});

it("only accepts fixed return destinations and keeps ordinary and adult browse scopes separate", () => {
  for (const destination of [
    "https://outside.test",
    { to: "https://outside.test" },
    { to: "media", media: "../private" },
    { to: "library", search: [] },
    { to: "library", search: { kind: "adult_video" } },
  ]) {
    expect(
      validateProviderSetupSearch({
        from: "library",
        kind: "movie",
        return: destination,
      }),
    ).not.toHaveProperty("return");
  }
  expect(
    validateProviderSetupSearch({
      from: "library",
      kind: "adult_video",
      return: { to: "library", search: { kind: "movie" } },
    }),
  ).not.toHaveProperty("return");
  expect(
    validateProviderSetupSearch({
      from: "scraper",
      media: "media-1",
      return: { to: "roots" },
    }),
  ).not.toHaveProperty("return");
});

it("returns to the original directory list or exact media with the correct privacy scope", () => {
  expect(
    setupReturnLocation(
      validateProviderSetupSearch({
        from: "library",
        kind: "adult_video",
        return: { to: "roots" },
      }),
    ),
  ).toEqual({ to: "/roots", search: { kind: "adult_video" } });
  expect(
    setupReturnLocation(
      validateProviderSetupSearch({
        from: "library",
        kind: "adult_video",
        return: { to: "media", media: "media-1" },
      }),
    ),
  ).toEqual({
    to: "/media/$mediaId",
    params: { mediaId: "media-1" },
    search: { kind: "adult_video" },
  });
});
it("retains the validated manual review queue through source setup without mixing root context", () => {
  expect(
    validateProviderSetupSearch({
      from: "scraper",
      media: "media-2",
      queue: ["media-1", "media-2"],
      root: "other",
      tab: "providers",
    }),
  ).toEqual({
    from: "scraper",
    media: "media-2",
    queue: ["media-1", "media-2"],
    tab: "providers",
  });
});
