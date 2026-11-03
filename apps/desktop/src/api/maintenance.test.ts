import { invoke, isTauri } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { databaseMaintenanceApi as api } from "./maintenance";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), isTauri: vi.fn() }));
const backup = {
  id: "backup-id",
  created_at: "2026-11-03T00:00:00Z",
  app_version: "0.1.0-beta.1",
  schema_version: 17,
  bytes: 4096,
  contains_private_metadata: false,
};
beforeEach(() => {
  vi.mocked(isTauri).mockReturnValue(true);
  vi.mocked(invoke).mockReset();
});
describe("database maintenance IPC", () => {
  it("requires desktop and validates safe status envelopes", async () => {
    vi.mocked(isTauri).mockReturnValue(false);
    await expect(api.status()).rejects.toMatchObject({
      code: "desktop_required",
    });
    expect(invoke).not.toHaveBeenCalled();
    vi.mocked(isTauri).mockReturnValue(true);
    vi.mocked(invoke).mockResolvedValue({
      schema_version: 1,
      data: {
        backups: [backup],
        restore_pending: false,
        directory: "/synthetic/maintenance",
      },
    });
    expect((await api.status()).backups).toEqual([backup]);
    vi.mocked(invoke).mockResolvedValue({
      schema_version: 1,
      data: {
        backups: [{ ...backup, bytes: -1 }],
        restore_pending: false,
        directory: "local",
      },
    });
    await expect(api.status()).rejects.toMatchObject({
      code: "invalid_response",
    });
  });
  it("only confirms backend preview IDs and demands restart acknowledgement", async () => {
    vi.mocked(invoke).mockResolvedValue({
      schema_version: 1,
      data: { restart_required: true },
    });
    await api.restoreConfirm("immutable-preview");
    expect(invoke).toHaveBeenLastCalledWith("maintenance_restore_confirm", {
      input: { id: "immutable-preview" },
    });
    vi.mocked(invoke).mockResolvedValue({
      schema_version: 1,
      data: { restart_required: false },
    });
    await expect(api.restoreConfirm("immutable-preview")).rejects.toMatchObject(
      { code: "invalid_response" },
    );
  });
  it("rejects a backup preview claiming to include secrets", async () => {
    vi.mocked(invoke).mockResolvedValue({
      schema_version: 1,
      data: {
        id: "x",
        contains_private_metadata: true,
        includes_secrets: true,
        includes_source_media: false,
        may_include_tag_recovery_bytes: true,
      },
    });
    await expect(api.backupPreview()).rejects.toMatchObject({
      code: "invalid_response",
    });
  });
});
