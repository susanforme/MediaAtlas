import { invoke, isTauri } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { defaultBrowse, libraryApi } from "./library";

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
});
