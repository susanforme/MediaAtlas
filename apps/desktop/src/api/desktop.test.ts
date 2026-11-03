import { invoke, isTauri } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { DesktopError, getAppInfo } from "./desktop";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), isTauri: vi.fn() }));

describe("getAppInfo IPC contract", () => {
  beforeEach(() => {
    vi.mocked(isTauri).mockReturnValue(true);
  });

  it("does not fake a successful backend connection in browser preview", async () => {
    vi.mocked(isTauri).mockReturnValue(false);
    await expect(getAppInfo()).rejects.toMatchObject({
      code: "desktop_required",
    });
    expect(invoke).not.toHaveBeenCalled();
  });

  it("reads a versioned response from the diagnostic command", async () => {
    const info = {
      schema_version: 1,
      app_name: "MediaAtlas",
      app_version: "0.1.0",
    };
    vi.mocked(invoke).mockResolvedValue(info);
    await expect(getAppInfo()).resolves.toEqual(info);
    expect(invoke).toHaveBeenCalledWith("get_app_info");
  });

  it.each([
    { schema_version: 2, app_name: "MediaAtlas", app_version: "0.1.0" },
    { schema_version: 1, app_name: "WrongApp", app_version: "0.1.0" },
    { schema_version: 1, app_name: "MediaAtlas", app_version: 123 },
    null,
  ])("rejects an incompatible or malformed response: %j", async (value) => {
    vi.mocked(invoke).mockResolvedValue(value);
    await expect(getAppInfo()).rejects.toMatchObject({
      code: "invalid_response",
    });
  });

  it("preserves a valid backend diagnostic reference", async () => {
    const envelope = {
      schema_version: 1,
      code: "internal_error",
      message: "Unavailable",
      correlation_id: "ref-123",
    };
    vi.mocked(invoke).mockRejectedValue(envelope);
    await expect(getAppInfo()).rejects.toMatchObject({
      code: "request_failed",
      envelope,
    });
  });

  it("does not expose raw transport error content", async () => {
    vi.mocked(invoke).mockRejectedValue("token=secret /Users/private");
    try {
      await getAppInfo();
      expect.fail("Expected a DesktopError");
    } catch (error) {
      expect(error).toBeInstanceOf(DesktopError);
      expect(error).toMatchObject({
        code: "request_failed",
        envelope: undefined,
      });
      expect(String(error)).not.toContain("secret");
    }
  });
});
