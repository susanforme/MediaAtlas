import { invoke, isTauri } from "@tauri-apps/api/core";
import { beforeEach, expect, it, vi } from "vitest";
import { adultApi } from "./adult";
import { defaultBrowse, libraryApi } from "./library";
import {
  maintenanceApi,
  nativeOpenApi,
  organizationApi,
  organizationPlan,
} from "./organization";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), isTauri: vi.fn() }));
beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(isTauri).mockReturnValue(true);
});
it("dispatches only backend file IDs through distinct native lanes", async () => {
  vi.mocked(invoke).mockResolvedValue({
    schema_version: 1,
    data: { requested: true },
  });
  await nativeOpenApi.open("file");
  expect(invoke).toHaveBeenLastCalledWith("library_open", {
    input: { file_id: "file" },
  });
  await nativeOpenApi.open("private", true);
  expect(invoke).toHaveBeenLastCalledWith("adult_open", {
    input: { file_id: "private" },
  });
  vi.mocked(invoke).mockResolvedValue({
    schema_version: 1,
    data: { requested: false },
  });
  await expect(nativeOpenApi.open("file")).rejects.toMatchObject({
    code: "invalid_response",
  });
});
it("keeps unfinished resources available only through separate tools endpoints", async () => {
  vi.mocked(invoke).mockResolvedValue({
    schema_version: 1,
    data: { items: [], next_cursor: null },
  });
  await libraryApi.toolsMedia(defaultBrowse());
  expect(invoke).toHaveBeenLastCalledWith("library_tools_media", {
    input: defaultBrowse(),
  });
  await adultApi.toolsMedia(defaultBrowse(true));
  expect(invoke).toHaveBeenLastCalledWith("adult_tools_media", {
    input: defaultBrowse(true),
  });
});
it("validates immutable plans and keeps execution explicit", async () => {
  const plan = {
    id: "plan",
    media_item_id: "media",
    target_root_id: "root",
    strategy: "preserve",
    state: "planned",
    files: [
      {
        source: "a.mkv",
        target: "b.mkv",
        role: "media",
        bytes: 2,
        conflict: null,
        same_path: false,
      },
    ],
    estimated_bytes: 0,
    file_count: 1,
    warnings: [],
  };
  expect(organizationPlan(plan)).toBe(true);
  expect(
    organizationPlan({ ...plan, files: [{ ...plan.files[0], bytes: -1 }] }),
  ).toBe(false);
  vi.mocked(invoke).mockResolvedValue({ schema_version: 1, data: plan });
  await organizationApi().preview({
    media_item_id: "media",
    target_root_id: "root",
    strategy: "preserve",
    template: "{stem}.{ext}",
  });
  expect(invoke).toHaveBeenCalledOnce();
  expect(invoke).toHaveBeenLastCalledWith("organization_preview", {
    input: {
      media_item_id: "media",
      target_root_id: "root",
      strategy: "preserve",
      template: "{stem}.{ext}",
    },
  });
  vi.mocked(invoke).mockResolvedValue({
    schema_version: 1,
    data: {
      id: "plan",
      state: "succeeded",
      completed: 1,
      already_applied: false,
      error_code: null,
    },
  });
  await organizationApi(true).rollbackExecute("plan");
  expect(invoke).toHaveBeenLastCalledWith(
    "adult_organization_rollback_execute",
    { input: { id: "plan" } },
  );
});
it("validates watcher and bounded health response contracts", async () => {
  vi.mocked(invoke).mockResolvedValue({
    schema_version: 1,
    data: {
      root_id: "root",
      opted_in: true,
      active: true,
      pending_rescan: false,
      last_task_id: null,
      error_code: null,
    },
  });
  await maintenanceApi.configure("root", true);
  expect(invoke).toHaveBeenLastCalledWith("watcher_configure", {
    input: { root_id: "root", enabled: true },
  });
  vi.mocked(invoke).mockResolvedValue({
    schema_version: 1,
    data: { items: [], next_cursor: null },
  });
  await maintenanceApi.health();
  expect(invoke).toHaveBeenLastCalledWith("health_check", {
    input: { cursor: null, limit: 50 },
  });
});
