import { invoke, isTauri } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { fixtureSnapshot, fixtureTask } from "../features/providers/fixtures";
import { providerApi } from "./providers";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), isTauri: vi.fn() }));
function respond(data: unknown, version = 1) {
  vi.mocked(invoke).mockResolvedValue({ schema_version: version, data });
}
beforeEach(() => {
  vi.mocked(isTauri).mockReturnValue(true);
});
describe("provider IPC validation", () => {
  it("accepts a versioned snapshot and sends explicit input envelopes", async () => {
    const data = fixtureSnapshot();
    data.tasks = [fixtureTask];
    data.requests = [
      {
        task_id: "task-1",
        provider_id: "fixture.offline",
        engine: "api",
        method: "FIXTURE",
        safe_url: "fixture://offline",
        summary: "fixture_json",
        duration_ms: 1,
        status: null,
        failure: null,
        attempt: 1,
      },
    ];
    respond(data);
    expect(await providerApi.snapshot()).toEqual(data);
    expect(invoke).toHaveBeenCalledWith("provider_snapshot", { input: {} });
    respond(null);
    await providerApi.setEnabled("fixture.offline", false);
    expect(invoke).toHaveBeenLastCalledWith("provider_set_enabled", {
      input: { provider_id: "fixture.offline", enabled: false },
    });
    await providerApi.updateConfig("fixture.offline", data.providers[0].config);
    expect(invoke).toHaveBeenLastCalledWith("provider_update_config", {
      input: {
        provider_id: "fixture.offline",
        config: data.providers[0].config,
      },
    });
    respond("task-1");
    expect(await providerApi.start("fixture.offline", "Title", "en-US")).toBe(
      "task-1",
    );
    expect(invoke).toHaveBeenLastCalledWith("provider_start", {
      input: {
        provider_id: "fixture.offline",
        query: "Title",
        language: "en-US",
      },
    });
    respond(true);
    expect(await providerApi.cancel("task-1")).toBe(true);
    expect(invoke).toHaveBeenLastCalledWith("provider_cancel", {
      input: { task_id: "task-1" },
    });
  });
  it("rejects future schemas, missing fields and invalid failures", async () => {
    respond(fixtureSnapshot(), 2);
    await expect(providerApi.snapshot()).rejects.toMatchObject({
      code: "invalid_response",
    });
    respond({ ...fixtureSnapshot(), schema_version: 2 });
    await expect(providerApi.snapshot()).rejects.toMatchObject({
      code: "invalid_response",
    });
    const data = fixtureSnapshot();
    delete (
      data.providers[0].config as Partial<(typeof data.providers)[0]["config"]>
    ).timeout_ms;
    respond(data);
    await expect(providerApi.snapshot()).rejects.toMatchObject({
      code: "invalid_response",
    });
    respond({
      ...fixtureSnapshot(),
      tasks: [{ ...fixtureTask, failure: "raw secret" }],
    });
    await expect(providerApi.snapshot()).rejects.toMatchObject({
      code: "invalid_response",
    });
  });
  it.each([
    "https://user:secret@site.example/",
    "https://site.example/private",
    "https://site.example/?token=secret",
    "https://site.example/#secret",
    "http://localhost/private",
  ])("refuses non-redacted network URLs %s", async (url) => {
    respond({
      ...fixtureSnapshot(),
      requests: [
        {
          task_id: "x",
          provider_id: "fixture.offline",
          engine: "api",
          method: "GET",
          safe_url: url,
          duration_ms: 1,
          status: 200,
          failure: null,
          summary: null,
          attempt: 1,
        },
      ],
    });
    await expect(providerApi.snapshot()).rejects.toMatchObject({
      code: "invalid_response",
    });
  });
  it("keeps browser mode explicit and does not expose raw backend errors", async () => {
    vi.mocked(isTauri).mockReturnValue(false);
    await expect(providerApi.snapshot()).rejects.toMatchObject({
      code: "desktop_required",
    });
    expect(invoke).not.toHaveBeenCalled();
    vi.mocked(isTauri).mockReturnValue(true);
    vi.mocked(invoke).mockRejectedValue("token=secret");
    await expect(providerApi.snapshot()).rejects.toMatchObject({
      code: "request_failed",
      envelope: undefined,
    });
  });
});

it.each(["[redacted]", "http://localhost/", "https://example.test"])(
  "preserves a safe denied-target record %s",
  async (safe_url) => {
    respond({
      ...fixtureSnapshot(),
      requests: [
        {
          task_id: "x",
          provider_id: "fixture.offline",
          engine: "api",
          method: "GET",
          safe_url,
          duration_ms: 1,
          status: null,
          failure: "unsafe_target",
          summary: null,
          attempt: 1,
        },
      ],
    });
    const value = await providerApi.snapshot();
    expect(value.requests[0].failure).toBe("unsafe_target");
  },
);

it("sends the selected Chinese locale in the immutable job input", async () => {
  respond("task-zh");
  await providerApi.start("fixture.offline", "中文检索", "zh-CN");
  expect(invoke).toHaveBeenLastCalledWith("provider_start", {
    input: {
      provider_id: "fixture.offline",
      query: "中文检索",
      language: "zh-CN",
    },
  });
});
