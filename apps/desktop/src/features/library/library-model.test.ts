import { expect, it } from "vitest";
import type { RootView } from "../../api/library";
import { newRootInput, normalizeRootInput, rootInputFromView, rootSupportsNfoExport } from "./library-model";

it("defaults old directories to HTTPS and carries an explicit saved NFO mode into their editor", () => {
  const root: RootView = {
    id: "films", name: "Films", path: "/films", path_style: "posix", kind: "movie",
    enabled: true, exclude_patterns: [], created_at: "2026-11-05", updated_at: "2026-11-05", last_scan_at: null,
  };
  expect(newRootInput(false).nfo_export_mode).toBe("online");
  expect(rootInputFromView(root).nfo_export_mode).toBe("online");
  expect(rootInputFromView({ ...root, nfo_export_mode: "offline" }).nfo_export_mode).toBe("offline");
  expect(rootInputFromView(root).nfo_export_mode).toBe("online");
});

it.each([null, "movie", "tv_series", "anime"] as const)("keeps offline NFO output for the supported %s directory", (kind) => {
  const form = { ...newRootInput(false), kind, nfo_export_mode: "offline" as const };
  expect(rootSupportsNfoExport(kind)).toBe(true);
  expect(normalizeRootInput(form, "").nfo_export_mode).toBe("offline");
});

it.each(["music_release", "adult_video"] as const)("does not carry offline NFO output into a %s directory", (kind) => {
  const form = { ...newRootInput(false), kind, nfo_export_mode: "offline" as const };
  expect(rootSupportsNfoExport(kind)).toBe(false);
  expect(normalizeRootInput(form, "").nfo_export_mode).toBe("online");
  expect(form.nfo_export_mode).toBe("offline");
});
