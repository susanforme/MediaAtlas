import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { I18nextProvider } from "react-i18next";
import { beforeEach, expect, it, vi } from "vitest";
import { libraryApi, type RootView, type ScanTask } from "../../api/library";
import { type AuthStatus, providerAuthApi } from "../../api/provider-auth";
import { providerApi } from "../../api/providers";
import { toast } from "../../components/ui/sonner";
import i18n from "../../i18n";
import { chooseSelectOption } from "../../test/select";
import { fixtureSnapshot } from "../providers/fixtures";
import { ProviderAuthSettings } from "./provider-auth";
import type { ProviderSetupSearch } from "./setup-search";

vi.mock("../../components/ui/sonner", () => ({
  toast: { success: vi.fn(), info: vi.fn(), error: vi.fn() },
}));
vi.mock("../library/context", () => ({
  usePrivacy: () => ({ includeAdult: true, settings: { entry_visible: true } }),
}));
vi.mock("../../api/library", async (original) => ({
  ...(await original<typeof import("../../api/library")>()),
  libraryApi: { roots: vi.fn(), sync: vi.fn() },
}));
vi.mock("../../api/provider-auth", async (original) => ({
  ...(await original<typeof import("../../api/provider-auth")>()),
  providerAuthApi: { status: vi.fn(), save: vi.fn(), clear: vi.fn() },
}));
vi.mock("../../api/providers", async (original) => ({
  ...(await original<typeof import("../../api/providers")>()),
  providerApi: { snapshot: vi.fn(), setEnabled: vi.fn() },
}));

const folder: RootView = {
  id: "folder-1",
  name: "Movies",
  path: "/fixture/movies",
  path_style: "posix",
  kind: "movie",
  enabled: true,
  exclude_patterns: [],
  created_at: "2026-11-04T00:00:00Z",
  updated_at: "2026-11-04T00:00:00Z",
  last_scan_at: null,
};
const auth: AuthStatus = {
  provider_id: "tmdb",
  supported_methods: ["bearer", "api_key"],
  method: "bearer",
  storage: "system",
  configured: false,
  status: "not_configured",
};
const task: ScanTask = {
  id: "sync-1",
  root_id: folder.id,
  state: "queued",
  mode: "automatic",
  phase: "scan",
  visited_files: 0,
  skipped_files: 0,
  candidate_count: 0,
  imported_count: 0,
  matched_count: 0,
  review_count: 0,
  metadata_failed_count: 0,
  warnings: 0,
  error_code: null,
  created_at: folder.created_at,
  updated_at: folder.updated_at,
};
let enabled = false;
let events: string[] = [];
const setup: ProviderSetupSearch = {
  from: "library",
  root: folder.id,
  kind: "movie",
  provider: "tmdb",
};
function snapshot() {
  const value = fixtureSnapshot();
  value.providers[0].descriptor = {
    ...value.providers[0].descriptor,
    id: "tmdb",
    name: "TMDB",
    fixture: false,
    requires_api_key: true,
  };
  value.providers[0].config.enabled = enabled;
  return value;
}
function show(context = setup, complete = vi.fn()) {
  return {
    complete,
    ...render(
      <I18nextProvider i18n={i18n}>
        <QueryClientProvider
          client={
            new QueryClient({ defaultOptions: { queries: { retry: false } } })
          }
        >
          <ProviderAuthSettings setup={context} onSetupComplete={complete} />
        </QueryClientProvider>
      </I18nextProvider>,
    ),
  };
}
async function enter() {
  fireEvent.change(await screen.findByLabelText("Credential"), {
    target: { value: "synthetic-test-secret" },
  });
  await screen.findByText("Source disabled");
  events = [];
}
beforeEach(async () => {
  vi.clearAllMocks();
  enabled = false;
  events = [];
  await i18n.changeLanguage("en-US");
  vi.mocked(libraryApi.roots).mockResolvedValue([folder]);
  vi.mocked(libraryApi.sync).mockImplementation(async () => {
    events.push("sync");
    return task;
  });
  vi.mocked(providerAuthApi.status).mockImplementation(async (id) =>
    id === "tmdb"
      ? auth
      : {
          ...auth,
          provider_id: id,
          supported_methods: ["none"],
          method: "none",
          configured: true,
          status: "not_required",
        },
  );
  vi.mocked(providerAuthApi.save).mockImplementation(async () => {
    events.push("save");
    return { ...auth, configured: true, status: "configured" };
  });
  vi.mocked(providerApi.snapshot).mockImplementation(async () => {
    events.push("verify");
    return snapshot();
  });
  vi.mocked(providerApi.setEnabled).mockImplementation(async () => {
    events.push("enable");
    enabled = true;
    return null;
  });
});

it("only performs save, enable, verified source state and the same folder refresh after the explicit combined action", async () => {
  const view = show();
  await enter();
  expect(providerApi.setEnabled).not.toHaveBeenCalled();
  expect(libraryApi.sync).not.toHaveBeenCalled();
  fireEvent.click(
    screen.getByRole("button", { name: "Save, enable and continue updating" }),
  );
  expect(screen.getByLabelText("Credential")).toHaveValue("");
  await waitFor(() =>
    expect(view.complete).toHaveBeenCalledWith(folder.id, task.id, "movie"),
  );
  expect(events).toEqual(["save", "enable", "verify", "sync"]);
  expect(toast.success).toHaveBeenCalledExactlyOnceWith(
    "Library update started.",
  );
  expect(libraryApi.sync).toHaveBeenCalledWith(folder.id);
  expect(
    screen.queryByDisplayValue("synthetic-test-secret"),
  ).not.toBeInTheDocument();
});
it("stops before enable and scan if credential saving fails", async () => {
  vi.mocked(providerAuthApi.save).mockRejectedValue(
    new Error("synthetic failure"),
  );
  show();
  await enter();
  fireEvent.click(
    screen.getByRole("button", { name: "Save, enable and continue updating" }),
  );
  await screen.findByText(/Authentication was not saved/);
  expect(providerApi.setEnabled).not.toHaveBeenCalled();
  expect(libraryApi.sync).not.toHaveBeenCalled();
});
it("retries enable without asking for or saving the successfully stored secret again", async () => {
  vi.mocked(providerApi.setEnabled).mockRejectedValueOnce(
    new Error("enable failed"),
  );
  const view = show();
  await enter();
  fireEvent.click(
    screen.getByRole("button", { name: "Save, enable and continue updating" }),
  );
  await screen.findByText(
    "Could not confirm that the source is enabled. Refresh its status and retry.",
  );
  expect(libraryApi.sync).not.toHaveBeenCalled();
  fireEvent.click(
    screen.getByRole("button", { name: "Enable and continue updating" }),
  );
  await waitFor(() => expect(view.complete).toHaveBeenCalled());
  expect(providerAuthApi.save).toHaveBeenCalledTimes(1);
  expect(providerApi.setEnabled).toHaveBeenCalledTimes(2);
});
it("does not claim success or scan when enabling succeeds but the confirmed source is still disabled", async () => {
  vi.mocked(providerApi.setEnabled).mockResolvedValue(null);
  const view = show();
  await enter();
  fireEvent.click(
    screen.getByRole("button", { name: "Save, enable and continue updating" }),
  );
  await screen.findByText(
    "Could not confirm that the source is enabled. Refresh its status and retry.",
  );
  expect(libraryApi.sync).not.toHaveBeenCalled();
  expect(view.complete).not.toHaveBeenCalled();
});
it("does not re-save or re-enable after a scan startup failure", async () => {
  vi.mocked(libraryApi.sync).mockRejectedValueOnce(new Error("scan failed"));
  const view = show();
  await enter();
  fireEvent.click(
    screen.getByRole("button", { name: "Save, enable and continue updating" }),
  );
  await screen.findByText(
    "The source is enabled, but the library update could not start. Retry the update.",
  );
  fireEvent.click(
    screen.getByRole("button", { name: "Continue updating this folder" }),
  );
  await waitFor(() => expect(view.complete).toHaveBeenCalled());
  expect(providerAuthApi.save).toHaveBeenCalledTimes(1);
  expect(providerApi.setEnabled).toHaveBeenCalledTimes(1);
});
it("does not enable or navigate after a late save when the user changes the selected source", async () => {
  let resolve!: (value: AuthStatus) => void;
  vi.mocked(providerAuthApi.save).mockReturnValue(
    new Promise((done) => {
      resolve = done;
    }),
  );
  const view = show();
  await enter();
  fireEvent.click(
    screen.getByRole("button", { name: "Save, enable and continue updating" }),
  );
  chooseSelectOption(screen.getByLabelText("Authentication provider"), "anilist");
  await act(async () =>
    resolve({ ...auth, configured: true, status: "configured" }),
  );
  expect(providerApi.setEnabled).not.toHaveBeenCalled();
  expect(libraryApi.sync).not.toHaveBeenCalled();
  expect(view.complete).not.toHaveBeenCalled();
});
it("refuses a missing directory and never starts a replacement or guessed target", async () => {
  vi.mocked(libraryApi.roots).mockResolvedValue([]);
  show();
  await screen.findByText("This folder is disabled or unavailable.");
  expect(providerApi.setEnabled).not.toHaveBeenCalled();
  expect(libraryApi.sync).not.toHaveBeenCalled();
});

it("keeps the ordinary authentication save separate from enablement and folder updates", async () => {
  show();
  await enter();
  fireEvent.click(screen.getByRole("button", { name: "Save authentication" }));
  await waitFor(() =>
    expect(toast.success).toHaveBeenCalledWith(
      "Authentication saved. Provider enablement is unchanged.",
    ),
  );
  expect(providerApi.setEnabled).not.toHaveBeenCalled();
  expect(libraryApi.sync).not.toHaveBeenCalled();
});

it("does not promise or start a folder update without a specific root", async () => {
  const view = show({ from: "library", kind: "movie", provider: "tmdb" });
  await enter();
  expect(
    screen.queryByRole("button", {
      name: "Save, enable and continue updating",
    }),
  ).not.toBeInTheDocument();
  fireEvent.click(
    screen.getByRole("button", { name: "Save and enable source" }),
  );
  await waitFor(() =>
    expect(view.complete).toHaveBeenCalledWith(undefined, undefined, "movie"),
  );
  expect(libraryApi.sync).not.toHaveBeenCalled();
});

it("enables a real public music source without calling unsupported credential APIs", async () => {
  vi.mocked(libraryApi.roots).mockResolvedValue([
    { ...folder, kind: "music_release" },
  ]);
  vi.mocked(providerApi.snapshot).mockImplementation(async () => {
    const result = snapshot();
    result.providers[0].descriptor = {
      ...result.providers[0].descriptor,
      id: "musicbrainz",
      name: "MusicBrainz",
      requires_api_key: false,
      media_kinds: ["music_release"],
    };
    return result;
  });
  const view = show({
    ...setup,
    kind: "music_release",
    provider: "musicbrainz",
  });
  fireEvent.click(
    await screen.findByRole("button", { name: "Enable and continue updating" }),
  );
  await waitFor(() =>
    expect(view.complete).toHaveBeenCalledWith(
      folder.id,
      task.id,
      "music_release",
    ),
  );
  expect(providerApi.setEnabled).toHaveBeenCalledWith("musicbrainz", true);
  expect(providerAuthApi.status).not.toHaveBeenCalled();
  expect(providerAuthApi.save).not.toHaveBeenCalled();
});

it("does not offer setup success when the requested source is absent", async () => {
  vi.mocked(providerApi.snapshot).mockResolvedValue({
    ...snapshot(),
    providers: [],
  });
  show();
  await screen.findByText(
    "This source is unavailable. Choose another source or refresh.",
  );
  expect(
    screen.queryByRole("button", { name: "Enable and continue updating" }),
  ).not.toBeInTheDocument();
  expect(providerApi.setEnabled).not.toHaveBeenCalled();
  expect(libraryApi.sync).not.toHaveBeenCalled();
});
