import { invoke, isTauri } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { defaultBrowse, libraryApi, scanTaskGuard } from "./library";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), isTauri: vi.fn() }));
describe("library IPC contracts", () => {
  beforeEach(() => {
    vi.mocked(isTauri).mockReturnValue(true);
  });
  it("uses versioned responses and explicit ordinary-media filtering", async () => {
    vi.mocked(invoke).mockResolvedValue({
      schema_version: 1,
      data: { items: [], next_cursor: null },
    });
    const query = defaultBrowse();
    await libraryApi.media(query);
    expect(invoke).toHaveBeenCalledWith("library_list_media", {
      input: { ...query, include_adult: false },
    });
    expect(query.limit).toBeLessThanOrEqual(200);
  });
  it("rejects an incompatible schema or malformed response", async () => {
    vi.mocked(invoke).mockResolvedValue({ schema_version: 2, data: [] });
    await expect(libraryApi.roots(false)).rejects.toMatchObject({
      code: "invalid_response",
    });
    vi.mocked(invoke).mockResolvedValue({ schema_version: 1, data: {} });
    await expect(libraryApi.roots(false)).rejects.toMatchObject({
      code: "invalid_response",
    });
  });
  it("does not invoke desktop commands in ordinary browser previews", async () => {
    vi.mocked(isTauri).mockReturnValue(false);
    await expect(libraryApi.tasks(false)).rejects.toMatchObject({
      code: "desktop_required",
    });
    expect(invoke).not.toHaveBeenCalled();
  });
  it("preserves directory source order and rejects malformed source arrays", async () => {
    const root = {
      id: "root", name: "TV", path: "/media", path_style: "posix",
      kind: "tv_series", enabled: true, exclude_patterns: [],
      created_at: "2026-11-04", updated_at: "2026-11-04", last_scan_at: null,
      metadata_providers: ["tvmaze", "tmdb"],
    };
    vi.mocked(invoke).mockResolvedValue({ schema_version: 1, data: [root] });
    await expect(libraryApi.roots(false)).resolves.toEqual([root]);
    vi.mocked(invoke).mockResolvedValue({
      schema_version: 1, data: [{ ...root, metadata_providers: ["tmdb", 1] }],
    });
    await expect(libraryApi.roots(false)).rejects.toMatchObject({ code: "invalid_response" });
  });
  it("starts background library updates and validates their actual progress", async () => {
    const task = {
      id: "task",
      root_id: "root",
      state: "queued",
      visited_files: 0,
      skipped_files: 0,
      candidate_count: 0,
      imported_count: 0,
      warnings: 0,
      error_code: null,
      created_at: "2026-11-03",
      updated_at: "2026-11-03",
      mode: "automatic",
      phase: "scan",
      matched_count: 0,
      review_count: 0,
      unconfigured_count: 0,
      metadata_failed_count: 0,
    };
    vi.mocked(invoke).mockResolvedValue({ schema_version: 1, data: task });
    await expect(libraryApi.sync("root")).resolves.toEqual(task);
    expect(invoke).toHaveBeenCalledWith("library_sync", {
      input: { root_id: "root" },
    });
    expect(scanTaskGuard({ ...task, phase: "pretend_done" })).toBe(false);
    expect(scanTaskGuard({ ...task, matched_count: -1 })).toBe(false);
    expect(scanTaskGuard({ ...task, review_count: undefined })).toBe(false);
    expect(scanTaskGuard({ ...task, unconfigured_count: undefined })).toBe(
      true,
    );
    expect(scanTaskGuard({ ...task, unconfigured_count: -1 })).toBe(false);
    expect(scanTaskGuard({ ...task, unconfigured_count: 1 })).toBe(false);
    expect(
      scanTaskGuard({ ...task, review_count: 2, unconfigured_count: 1 }),
    ).toBe(true);
    const {
      mode,
      phase,
      matched_count,
      review_count,
      unconfigured_count,
      metadata_failed_count,
      ...legacy
    } = task;
    expect(scanTaskGuard(legacy)).toBe(true);
  });
  it("round-trips a directory NFO mode and rejects unknown modes without breaking legacy roots", async () => {
    const root = {
      id: "root", name: "Films", path: "/media", path_style: "posix",
      kind: "movie", enabled: true, exclude_patterns: [],
      created_at: "2026-11-05", updated_at: "2026-11-05", last_scan_at: null,
    };
    vi.mocked(invoke).mockResolvedValue({ schema_version: 1, data: [root] });
    await expect(libraryApi.roots(false)).resolves.toEqual([root]);
    for (const mode of ["online", "offline"] as const) {
      const saved = { ...root, nfo_export_mode: mode };
      vi.mocked(invoke).mockResolvedValue({ schema_version: 1, data: saved });
      const input = { id: root.id, name: root.name, path: root.path,
        kind: "movie" as const, enabled: true, exclude_patterns: [], nfo_export_mode: mode };
      await expect(libraryApi.saveRoot(input)).resolves.toEqual(saved);
      expect(invoke).toHaveBeenLastCalledWith("library_save_root", { input });
    }
    for (const mode of ["invalid", null, 1]) {
      vi.mocked(invoke).mockResolvedValue({ schema_version: 1, data: [{ ...root, nfo_export_mode: mode }] });
      await expect(libraryApi.roots(false)).rejects.toMatchObject({ code: "invalid_response" });
    }
  });
});
