import { invoke, isTauri } from "@tauri-apps/api/core";
import { beforeEach, expect, it, vi } from "vitest";
import { DesktopError } from "./desktop";
import { isNfoStartup, nfoApi } from "./nfo";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), isTauri: vi.fn() }));
beforeEach(() => {
  vi.mocked(isTauri).mockReturnValue(true);
  vi.mocked(invoke).mockReset();
});

it("keeps browser startup independent of native commands", async () => {
  vi.mocked(isTauri).mockReturnValue(false);
  expect(await isNfoStartup()).toBe(false);
  expect(invoke).not.toHaveBeenCalled();
});

it("rejects malformed preview responses and never accepts a path from the webview", async () => {
  vi.mocked(invoke).mockResolvedValue({ schema_version: 1, data: { name: "private.nfo" } });
  await expect(nfoApi.takeOpened()).rejects.toEqual(new DesktopError("invalid_response"));
  expect(invoke).toHaveBeenCalledWith("nfo_take_opened", { input: {} });
});
