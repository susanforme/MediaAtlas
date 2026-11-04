import { expect, it } from "vitest";
import { validateProviderSetupSearch } from "./setup-search";

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
