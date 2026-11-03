import { invoke, isTauri } from "@tauri-apps/api/core";
import { beforeEach, expect, it, vi } from "vitest";
import { type AuthStatus, providerAuthApi } from "./provider-auth";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), isTauri: vi.fn() }));
const status: AuthStatus = {
  provider_id: "tmdb",
  supported_methods: ["bearer", "api_key"],
  method: "bearer",
  storage: "system",
  configured: false,
  status: "not_configured",
};
function respond(data: unknown) {
  vi.mocked(invoke).mockResolvedValue({ schema_version: 1, data });
}
beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(isTauri).mockReturnValue(true);
});
it("validates status and sends authentication only to the dedicated save command", async () => {
  respond(status);
  await expect(providerAuthApi.status("tmdb")).resolves.toEqual(status);
  expect(invoke).toHaveBeenLastCalledWith("provider_auth_status", {
    input: { provider_id: "tmdb" },
  });
  respond({
    ...status,
    configured: true,
    storage: "session_only",
    method: "api_key",
  });
  const input = {
    provider_id: "tmdb",
    method: "api_key" as const,
    storage: "session_only" as const,
    secret: "synthetic-test-key",
  };
  await providerAuthApi.save(input);
  expect(invoke).toHaveBeenLastCalledWith("provider_auth_save", { input });
  respond(status);
  await providerAuthApi.clear("tmdb");
  expect(invoke).toHaveBeenLastCalledWith("provider_auth_clear", {
    input: { provider_id: "tmdb" },
  });
});
it.each([
  { ...status, secret: "must-not-return" },
  { ...status, provider_id: "anilist" },
  { ...status, supported_methods: ["oauth"] },
  { ...status, storage: "plaintext" },
  { ...status, configured: "yes" },
])(
  "rejects secret-bearing, unsupported and mismatched status responses",
  async (data) => {
    respond(data);
    await expect(providerAuthApi.status("tmdb")).rejects.toMatchObject({
      code: "invalid_response",
    });
  },
);
it("does not echo raw save errors or invoke auth IPC from a browser", async () => {
  vi.mocked(invoke).mockRejectedValue("secret=synthetic-credential");
  await expect(
    providerAuthApi.save({
      provider_id: "tmdb",
      method: "bearer",
      storage: "system",
      secret: "synthetic-credential",
    }),
  ).rejects.toMatchObject({ code: "request_failed", envelope: undefined });
  vi.clearAllMocks();
  vi.mocked(isTauri).mockReturnValue(false);
  await expect(providerAuthApi.status("tmdb")).rejects.toMatchObject({
    code: "desktop_required",
  });
  expect(invoke).not.toHaveBeenCalled();
});
