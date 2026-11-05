import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { type ComponentProps, StrictMode } from "react";
import { I18nextProvider } from "react-i18next";
import { beforeEach, expect, it, vi } from "vitest";
import { adultApi } from "../../api/adult";
import { DesktopError } from "../../api/desktop";
import { pickDirectory } from "../../api/directories";
import {
  libraryApi,
  type RootInput,
  type RootView,
  type ScanTask,
} from "../../api/library";
import { providerAuthApi } from "../../api/provider-auth";
import { type ProviderView, providerApi } from "../../api/providers";
import { toast } from "../../components/ui/sonner";
import i18n from "../../i18n";
import { chooseSelectOption, selectOptions } from "../../test/select";

vi.mock("../../components/ui/sonner", () => ({
  toast: { success: vi.fn(), error: vi.fn() },
}));

import { PrivacyProvider } from "./context";
import { RootsPage } from "./pages";

const scope = vi.hoisted(() => ({ visible: false, allowed: true }));
vi.mock("../../api/directories", () => ({ pickDirectory: vi.fn() }));
vi.mock("./context", async (original) => ({
  ...(await original<typeof import("./context")>()),
  usePrivacy: () => ({
    includeAdult: scope.allowed,
    settings: { entry_visible: scope.visible },
  }),
}));
vi.mock("../../api/adult", async (original) => ({
  ...(await original<typeof import("../../api/adult")>()),
  adultApi: { roots: vi.fn(), tasks: vi.fn(), providers: vi.fn() },
}));
vi.mock("../../api/providers", () => ({ providerApi: { snapshot: vi.fn() } }));
vi.mock("../../api/provider-auth", () => ({
  providerAuthApi: { status: vi.fn() },
}));
vi.mock("@tanstack/react-router", () => ({
  Link: ({ to, children }: { to: string; children: React.ReactNode }) => (
    <a href={to}>{children}</a>
  ),
}));
vi.mock("../../api/library", async (original) => {
  const module = await original<typeof import("../../api/library")>();
  return {
    ...module,
    libraryApi: {
      ...module.libraryApi,
      roots: vi.fn(),
      tasks: vi.fn(),
      saveRoot: vi.fn(),
      start: vi.fn(),
      sync: vi.fn(),
      confirm: vi.fn(),
      removeRoot: vi.fn(),
    },
  };
});
const time = "2026-11-03T00:00:00Z";
const scanTask: ScanTask = {
  id: "fixture-scan",
  root_id: "fixture-root",
  state: "queued",
  visited_files: 0,
  skipped_files: 0,
  candidate_count: 0,
  imported_count: 0,
  warnings: 0,
  error_code: null,
  created_at: time,
  updated_at: time,
};
function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((done, fail) => {
    resolve = done;
    reject = fail;
  });
  return { promise, resolve, reject };
}
function show(props: ComponentProps<typeof RootsPage> = {}) {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <StrictMode>
      <I18nextProvider i18n={i18n}>
        <QueryClientProvider client={client}>
          <PrivacyProvider>
            <RootsPage {...props} />
          </PrivacyProvider>
        </QueryClientProvider>
      </I18nextProvider>
    </StrictMode>,
  );
}
beforeEach(async () => {
  localStorage.clear();
  await i18n.changeLanguage("en-US");
  vi.clearAllMocks();
  scope.visible = false;
  scope.allowed = true;
  vi.mocked(libraryApi.roots).mockResolvedValue([]);
  vi.mocked(adultApi.roots).mockResolvedValue([]);
  vi.mocked(libraryApi.tasks).mockResolvedValue([]);
  vi.mocked(adultApi.tasks).mockResolvedValue([]);
  const snapshot = {
    schema_version: 1 as const,
    providers: [],
    tasks: [],
    requests: [],
  };
  vi.mocked(providerApi.snapshot).mockResolvedValue(snapshot);
  vi.mocked(adultApi.providers).mockResolvedValue(snapshot);
  vi.mocked(pickDirectory).mockResolvedValue(null);
  vi.mocked(libraryApi.start).mockResolvedValue(scanTask);
  vi.mocked(libraryApi.sync).mockResolvedValue({
    ...scanTask,
    mode: "automatic",
    phase: "scan",
  });
  vi.mocked(libraryApi.saveRoot).mockImplementation(
    async (input: RootInput) =>
      ({
        ...input,
        id: "fixture-root",
        path_style: "posix",
        created_at: time,
        updated_at: time,
        last_scan_at: null,
      }) as RootView,
  );
});
it("submits an explicit root type and normalized exclusion lines", async () => {
  show();
  await screen.findByText("No library roots yet");
  fireEvent.click(screen.getByRole("button", { name: "Add root" }));
  fireEvent.change(screen.getByLabelText("Root name"), {
    target: { value: "Fixture root" },
  });
  fireEvent.click(screen.getByRole("button", { name: "Advanced settings" }));
  fireEvent.change(screen.getByLabelText("Absolute path"), {
    target: { value: "/fixture/media" },
  });
  fireEvent.change(
    screen.getByLabelText("Exclude patterns (one glob per line)"),
    { target: { value: "  **/sample/**\n\n**/*.tmp  " } },
  );
  await waitFor(() =>
    expect(
      screen.getByRole("button", { name: "Add to library" }),
    ).toBeEnabled(),
  );
  fireEvent.click(screen.getByRole("button", { name: "Add to library" }));
  await waitFor(() =>
    expect(libraryApi.saveRoot).toHaveBeenCalledWith({
      id: null,
      name: "Fixture root",
      path: "/fixture/media",
      kind: "movie",
      enabled: true,
      exclude_patterns: ["**/sample/**", "**/*.tmp"],
      metadata_providers: null,
      nfo_export_mode: "online",
    }),
  );
});
it("saves an explicitly chosen offline NFO mode before starting the directory's background update", async () => {
  vi.mocked(pickDirectory).mockResolvedValue("/media/Films");
  show();
  fireEvent.click(screen.getByRole("button", { name: "Add root" }));
  await screen.findByText("/media/Films");
  const mode = screen.getByLabelText("Default NFO export mode");
  await waitFor(() => expect(mode).toBeEnabled());
  expect(mode).toHaveValue("online");
  chooseSelectOption(mode, "offline");
  expect(screen.getByText("Generating missing NFOs in the background also writes their images. Existing NFOs are preserved and source videos stay read-only.")).toBeInTheDocument();
  expect(libraryApi.saveRoot).not.toHaveBeenCalled();
  expect(libraryApi.sync).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Add to library" }));
  await waitFor(() => expect(libraryApi.saveRoot).toHaveBeenCalledWith(
    expect.objectContaining({ path: "/media/Films", nfo_export_mode: "offline" }),
  ));
  await waitFor(() => expect(libraryApi.sync).toHaveBeenCalledExactlyOnceWith("fixture-root"));
});

it("reads each directory's saved NFO mode independently and discards cancelled edits", async () => {
  const first: RootView = {
    id: "first", name: "Offline films", path: "/media/Offline", path_style: "posix", kind: "movie",
    enabled: false, exclude_patterns: [], created_at: time, updated_at: time, last_scan_at: null,
    nfo_export_mode: "offline",
  };
  const second: RootView = { ...first, id: "second", name: "Linked films", path: "/media/Linked", nfo_export_mode: "online" };
  vi.mocked(libraryApi.roots).mockResolvedValue([first, second]);
  show();
  const firstCard = (await screen.findByText(first.name)).closest("article")!;
  const secondCard = screen.getByText(second.name).closest("article")!;
  fireEvent.click(within(firstCard).getByRole("button", { name: "Edit" }));
  expect(screen.getByLabelText("Default NFO export mode")).toHaveValue("offline");
  chooseSelectOption(screen.getByLabelText("Default NFO export mode"), "online");
  fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
  fireEvent.click(within(firstCard).getByRole("button", { name: "Edit" }));
  expect(screen.getByLabelText("Default NFO export mode")).toHaveValue("offline");
  fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
  fireEvent.click(within(secondCard).getByRole("button", { name: "Edit" }));
  expect(screen.getByLabelText("Default NFO export mode")).toHaveValue("online");
  expect(libraryApi.saveRoot).not.toHaveBeenCalled();
  chooseSelectOption(screen.getByLabelText("Default NFO export mode"), "offline");
  fireEvent.click(screen.getByRole("button", { name: "Save" }));
  await waitFor(() => expect(libraryApi.saveRoot).toHaveBeenCalledExactlyOnceWith(
    expect.objectContaining({ id: "second", nfo_export_mode: "offline" }),
  ));
  expect(first.nfo_export_mode).toBe("offline");
  expect(second.nfo_export_mode).toBe("online");
});

it.each(["music_release", "adult_video"] as const)("clears a pending offline mode when switching to %s", async (kind) => {
  scope.visible = true;
  show();
  fireEvent.click(screen.getByRole("button", { name: "Add root" }));
  const mode = await screen.findByLabelText("Default NFO export mode");
  await waitFor(() => expect(mode).toBeEnabled());
  chooseSelectOption(mode, "offline");
  chooseSelectOption(screen.getByLabelText("Root media type"), kind);
  expect(screen.queryByLabelText("Default NFO export mode")).not.toBeInTheDocument();
  chooseSelectOption(screen.getByLabelText("Root media type"), "movie");
  expect(screen.getByLabelText("Default NFO export mode")).toHaveValue("online");
  expect(libraryApi.saveRoot).not.toHaveBeenCalled();
});

it("uses localized validation instead of submitting empty root fields", async () => {
  show();
  await screen.findByText("No library roots yet");
  fireEvent.click(screen.getByRole("button", { name: "Add root" }));
  await waitFor(() =>
    expect(
      screen.getByRole("button", { name: "Add to library" }),
    ).toBeEnabled(),
  );
  fireEvent.click(screen.getByRole("button", { name: "Add to library" }));
  expect(await screen.findByRole("alert")).toHaveTextContent(
    "Enter a root name, absolute path and media type.",
  );
  expect(libraryApi.saveRoot).not.toHaveBeenCalled();
});

it("keeps root removal explicit and restores focus when its dialog is dismissed", async () => {
  vi.mocked(libraryApi.roots).mockResolvedValue([
    {
      id: "fixture-root",
      name: "Synthetic directory",
      path: "/fixture/media",
      path_style: "posix",
      kind: "movie",
      enabled: true,
      exclude_patterns: [],
      created_at: time,
      updated_at: time,
      last_scan_at: null,
    },
  ]);
  show();
  const trigger = await screen.findByRole("button", { name: "Remove" });
  trigger.focus();
  fireEvent.click(trigger);
  const dialog = await screen.findByRole("dialog", {
    name: "Remove root index",
  });
  expect(dialog).toHaveTextContent("Source files on disk will be preserved.");
  await waitFor(() =>
    expect(dialog).toContainElement(document.activeElement as HTMLElement),
  );
  fireEvent.keyDown(dialog, { key: "Escape" });
  await waitFor(() =>
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument(),
  );
  expect(trigger).toHaveFocus();
});

it.each([
  ["/Volumes/Media/Movies/", "Movies"],
  ["D:\\Media\\电影\\", "电影"],
  ["\\\\server\\share\\Series", "Series"],
])(
  "uses the folder chooser and derives a cross-platform name from %s",
  async (path, name) => {
    vi.mocked(pickDirectory).mockResolvedValue(path);
    show();
    fireEvent.click(screen.getByRole("button", { name: "Add root" }));
    expect(screen.queryByLabelText("Absolute path")).not.toBeInTheDocument();
    expect(
      screen.queryByLabelText("Exclude patterns (one glob per line)"),
    ).not.toBeInTheDocument();
    await waitFor(() =>
      expect(screen.getByLabelText("Root name")).toHaveValue(name),
    );
    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: "Add to library" }),
      ).toBeEnabled(),
    );
    fireEvent.click(screen.getByRole("button", { name: "Add to library" }));
    await waitFor(() =>
      expect(libraryApi.saveRoot).toHaveBeenCalledWith(
        expect.objectContaining({ path, name, kind: "movie" }),
      ),
    );
    expect(libraryApi.start).not.toHaveBeenCalled();
    expect(libraryApi.sync).toHaveBeenCalledExactlyOnceWith("fixture-root");
    await waitFor(() =>
      expect(toast.success).toHaveBeenCalledExactlyOnceWith(
        "Library update started in the background.",
      ),
    );
    expect(
      screen.queryByText("Library update started in the background."),
    ).not.toBeInTheDocument();
    expect(screen.getByRole("link", { name: "View library" })).toBeVisible();
  },
);

it("preserves an edited name and leaves the form untouched when the native picker is cancelled", async () => {
  vi.mocked(pickDirectory)
    .mockResolvedValueOnce("/media/Selected")
    .mockResolvedValueOnce(null);
  show();
  fireEvent.click(screen.getByRole("button", { name: "Add root" }));
  fireEvent.change(screen.getByLabelText("Root name"), {
    target: { value: "My archive" },
  });
  await screen.findByText("/media/Selected");
  expect(screen.getByLabelText("Root name")).toHaveValue("My archive");
  fireEvent.click(screen.getByRole("button", { name: "Change folder" }));
  await waitFor(() =>
    expect(screen.getByRole("button", { name: "Change folder" })).toBeEnabled(),
  );
  expect(pickDirectory).toHaveBeenLastCalledWith({
    title: "Choose folder",
    defaultPath: "/media/Selected",
  });
  expect(screen.getByLabelText("Root name")).toHaveValue("My archive");
  expect(screen.getByText("/media/Selected")).toBeInTheDocument();
  expect(libraryApi.saveRoot).not.toHaveBeenCalled();
});

it("ignores a late picker result after cancelling and opening another editor", async () => {
  const oldPicker = deferred<string | null>();
  vi.mocked(pickDirectory)
    .mockReturnValueOnce(oldPicker.promise)
    .mockResolvedValueOnce("/media/Current");
  show();
  fireEvent.click(screen.getByRole("button", { name: "Add root" }));
  expect(screen.getByRole("button", { name: "Add to library" })).toBeDisabled();
  expect(
    screen.queryByRole("button", { name: "Save and scan" }),
  ).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
  fireEvent.click(screen.getByRole("button", { name: "Add root" }));
  await screen.findByText("/media/Current");
  await act(async () => oldPicker.resolve("/media/Stale"));
  expect(screen.getByLabelText("Root name")).toHaveValue("Current");
  expect(screen.queryByText("/media/Stale")).not.toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Add to library" })).toBeEnabled();
});

it("keeps name edits made while a folder picker is pending", async () => {
  const picker = deferred<string | null>();
  vi.mocked(pickDirectory).mockReturnValue(picker.promise);
  show();
  fireEvent.click(screen.getByRole("button", { name: "Add root" }));
  fireEvent.change(screen.getByLabelText("Root name"), {
    target: { value: "Custom name" },
  });
  await act(async () => picker.resolve("C:\\Archive\\Movies"));
  expect(screen.getByLabelText("Root name")).toHaveValue("Custom name");
});

it("shows the typed picker error and keeps manual path entry available as a fallback", async () => {
  vi.mocked(pickDirectory).mockRejectedValue(
    new DesktopError("desktop_required"),
  );
  show();
  fireEvent.click(screen.getByRole("button", { name: "Add root" }));
  expect(await screen.findByRole("alert")).toHaveTextContent(
    "Open in the desktop app",
  );
  fireEvent.click(screen.getByRole("button", { name: "Advanced settings" }));
  expect(screen.getByLabelText("Absolute path")).toBeEnabled();
  expect(libraryApi.saveRoot).not.toHaveBeenCalled();
});

it("waits for saving before scanning, prevents duplicate submissions, and navigates with the real task", async () => {
  const pendingSave = deferred<RootView>();
  vi.mocked(pickDirectory).mockResolvedValue("/media/Movies");
  vi.mocked(libraryApi.saveRoot).mockReturnValue(pendingSave.promise);
  const onNavigateScan = vi.fn();
  show({ onNavigateScan });
  fireEvent.click(screen.getByRole("button", { name: "Add root" }));
  await screen.findByText("/media/Movies");
  const submit = screen.getByRole("button", { name: "Add to library" });
  fireEvent.click(submit);
  fireEvent.click(submit);
  await waitFor(() => expect(libraryApi.saveRoot).toHaveBeenCalledTimes(1));
  expect(libraryApi.sync).not.toHaveBeenCalled();
  expect(screen.getByRole("button", { name: "Change folder" })).toBeDisabled();
  await act(async () =>
    pendingSave.resolve({
      id: "fixture-root",
      name: "Movies",
      path: "/media/Movies",
      kind: "movie",
      enabled: true,
      exclude_patterns: [],
      path_style: "posix",
      created_at: time,
      updated_at: time,
      last_scan_at: null,
    }),
  );
  await waitFor(() =>
    expect(onNavigateScan).toHaveBeenCalledWith({
      root: "fixture-root",
      task: scanTask.id,
    }),
  );
  expect(libraryApi.sync).toHaveBeenCalledExactlyOnceWith("fixture-root");
  expect(libraryApi.confirm).not.toHaveBeenCalled();
});

it("retains a successfully saved root when scanning fails and retries only scanning", async () => {
  vi.mocked(pickDirectory).mockResolvedValue("/media/Movies");
  vi.mocked(libraryApi.sync)
    .mockRejectedValueOnce(new DesktopError("request_failed"))
    .mockResolvedValueOnce(scanTask);
  const onNavigateScan = vi.fn();
  show({ onNavigateScan });
  fireEvent.click(screen.getByRole("button", { name: "Add root" }));
  await screen.findByText("/media/Movies");
  await waitFor(() =>
    expect(
      screen.getByRole("button", { name: "Add to library" }),
    ).toBeEnabled(),
  );
  fireEvent.click(screen.getByRole("button", { name: "Add to library" }));
  expect(
    await screen.findByText(
      "The directory is saved, but the library update could not start.",
    ),
  ).toBeInTheDocument();
  expect(libraryApi.saveRoot).toHaveBeenCalledTimes(1);
  expect(onNavigateScan).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Retry update" }));
  await waitFor(() =>
    expect(onNavigateScan).toHaveBeenCalledWith({
      root: "fixture-root",
      task: scanTask.id,
    }),
  );
  expect(libraryApi.saveRoot).toHaveBeenCalledTimes(1);
  expect(libraryApi.sync).toHaveBeenCalledTimes(2);
});

it("adds an adult directory through the same form and starts the grant-bound automatic workflow", async () => {
  scope.visible = true;
  vi.mocked(pickDirectory).mockResolvedValue("/media/Private");
  const onNavigateScan = vi.fn();
  show({ onNavigateScan });
  fireEvent.click(screen.getByRole("button", { name: "Add root" }));
  await screen.findByText("/media/Private");
  chooseSelectOption(screen.getByRole("combobox", { name: "Root media type" }), "adult_video");
  await waitFor(() =>
    expect(
      screen.getByRole("button", { name: "Add to library" }),
    ).toBeEnabled(),
  );
  fireEvent.click(screen.getByRole("button", { name: "Add to library" }));
  await waitFor(() =>
    expect(onNavigateScan).toHaveBeenCalledWith({
      root: "fixture-root",
      task: scanTask.id,
      kind: "adult_video",
    }),
  );
  expect(libraryApi.sync).toHaveBeenCalledExactlyOnceWith("fixture-root");
  expect(libraryApi.start).not.toHaveBeenCalled();
  expect(libraryApi.confirm).not.toHaveBeenCalled();
  expect(libraryApi.saveRoot).toHaveBeenCalledWith(
    expect.objectContaining({ kind: "adult_video" }),
  );
});

it("submitting the directory form starts automatic discovery without a separate scan or import action", async () => {
  vi.mocked(pickDirectory).mockResolvedValue("/media/Movies");
  show();
  fireEvent.click(screen.getByRole("button", { name: "Add root" }));
  await screen.findByText("/media/Movies");
  const submit = screen.getByRole("button", { name: "Add to library" });
  expect(submit).toHaveAttribute("type", "submit");
  const form = submit.closest("form");
  expect(form).not.toBeNull();
  if (!form) throw new Error("Expected the directory form");
  fireEvent.submit(form);
  await waitFor(() =>
    expect(libraryApi.sync).toHaveBeenCalledExactlyOnceWith("fixture-root"),
  );
  expect(libraryApi.start).not.toHaveBeenCalled();
  expect(libraryApi.confirm).not.toHaveBeenCalled();
  expect(
    await screen.findByRole("link", { name: "View library" }),
  ).toHaveAttribute("href", "/library");
});

it("saves an explicitly disabled directory without starting background work", async () => {
  vi.mocked(pickDirectory).mockResolvedValue("/media/Offline");
  show();
  fireEvent.click(screen.getByRole("button", { name: "Add root" }));
  await screen.findByText("/media/Offline");
  fireEvent.click(screen.getByRole("button", { name: "Advanced settings" }));
  fireEvent.click(screen.getByRole("checkbox", { name: "Enabled" }));
  fireEvent.click(screen.getByRole("button", { name: "Save" }));
  await waitFor(() =>
    expect(libraryApi.saveRoot).toHaveBeenCalledWith(
      expect.objectContaining({ enabled: false }),
    ),
  );
  expect(libraryApi.sync).not.toHaveBeenCalled();
  expect(libraryApi.start).not.toHaveBeenCalled();
});

it("offers adult alongside ordinary types in one add form, without a separate scope selector", async () => {
  scope.visible = true;
  show();
  fireEvent.click(screen.getByRole("button", { name: "Add root" }));
  const types = screen.getByRole("combobox", { name: "Root media type" });
  const choices = selectOptions(types).map((option) => (option as HTMLButtonElement).value);
  expect(choices).toContain("movie");
  expect(choices).toContain("adult_video");
  expect(
    screen.queryByRole("combobox", { name: "Content type" }),
  ).not.toBeInTheDocument();
});

it("does not offer adult directories or fetch them when the entry is disabled", () => {
  show();
  fireEvent.click(screen.getByRole("button", { name: "Add root" }));
  const choices = selectOptions(screen.getByRole("combobox", { name: "Root media type" }));
  expect(choices.map((option) => (option as HTMLButtonElement).value)).not.toContain("adult_video");
  expect(adultApi.roots).not.toHaveBeenCalled();
});

it("does not save or synchronize an adult directory before the backend grant is ready", async () => {
  scope.visible = true;
  scope.allowed = false;
  vi.mocked(pickDirectory).mockResolvedValue("/media/Private");
  show();
  fireEvent.click(screen.getByRole("button", { name: "Add root" }));
  await screen.findByText("/media/Private");
  chooseSelectOption(screen.getByRole("combobox", { name: "Root media type" }), "adult_video");
  expect(screen.getByRole("button", { name: "Add to library" })).toBeDisabled();
  expect(adultApi.roots).not.toHaveBeenCalled();
  expect(libraryApi.saveRoot).not.toHaveBeenCalled();
  expect(libraryApi.sync).not.toHaveBeenCalled();
});

function source(id: string, requiresKey = false): ProviderView {
  return {
    descriptor: {
      id,
      name: id === "anilist" ? "AniList" : id.toUpperCase(),
      description: "",
      website: null,
      terms_url: null,
      privacy_url: null,
      license: "",
      documentation_url: null,
      engine_types: [],
      requires_api_key: requiresKey,
      requires_login: false,
      rate_limit_policy: "",
      attribution: "",
      data_usage_notes: "",
      media_kinds: id === "anilist" ? ["anime"] : ["dmm", "wikidata"].includes(id) ? ["adult_video"] : ["movie", "tv_series"],
      languages: [],
      capabilities: ["dmm", "wikidata"].includes(id) ? ["adult_search", "adult_detail"] : ["search", "details"],
      fixture: false,
    },
    config: {
      enabled: true,
      revision: 1,
      timeout_ms: 1000,
      concurrency: 1,
      image_concurrency: 1,
      interval_ms: 0,
      max_response_bytes: 1000,
      max_retries: 0,
      credential_ref: null,
    },
    health: "healthy",
  };
}

it("opens the system chooser once from the add route, preserving the requested media type", async () => {
  vi.mocked(pickDirectory).mockResolvedValue("/media/Anime");
  vi.mocked(providerApi.snapshot).mockResolvedValue({
    schema_version: 1,
    providers: [source("anilist")],
    tasks: [],
    requests: [],
  });
  show({ initialAdd: true, initialKind: "anime" });
  await screen.findByText("/media/Anime");
  expect(pickDirectory).toHaveBeenCalledTimes(1);
  expect(screen.getByRole("combobox", { name: "Root media type" })).toHaveValue(
    "anime",
  );
  expect(
    await screen.findByRole("checkbox", { name: "AniList" }),
  ).toBeChecked();
  expect(providerAuthApi.status).not.toHaveBeenCalled();
});

it("keeps source choices in the draft and cancels without saving or configuring the directory", async () => {
  vi.mocked(pickDirectory).mockResolvedValue("/media/Movies");
  const provider = source("tmdb");
  provider.config.enabled = false;
  vi.mocked(providerApi.snapshot).mockResolvedValue({
    schema_version: 1,
    providers: [provider],
    tasks: [],
    requests: [],
  });
  show({ initialAdd: true });
  await screen.findByText("/media/Movies");
  const sourceChoice = await screen.findByRole("checkbox", { name: "TMDB" });
  expect(sourceChoice).toBeChecked();
  expect(screen.getByText("Disabled")).toBeInTheDocument();
  expect(
    screen.queryByRole("button", { name: /Save and configure/ }),
  ).not.toBeInTheDocument();
  fireEvent.click(sourceChoice);
  expect(sourceChoice).not.toBeChecked();
  fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
  expect(libraryApi.saveRoot).not.toHaveBeenCalled();
  expect(libraryApi.sync).not.toHaveBeenCalled();
  expect(
    screen.queryByText("Metadata sources configured"),
  ).not.toBeInTheDocument();
});

it("allows local indexing with an unconfigured source and labels only the background start", async () => {
  vi.mocked(pickDirectory).mockResolvedValue("/media/Movies");
  const provider = source("tmdb");
  provider.config.enabled = false;
  vi.mocked(providerApi.snapshot).mockResolvedValue({
    schema_version: 1,
    providers: [provider],
    tasks: [],
    requests: [],
  });
  show({ initialAdd: true });
  await screen.findByText("/media/Movies");
  await screen.findByRole("checkbox", { name: "TMDB" });
  expect(libraryApi.saveRoot).not.toHaveBeenCalled();
  expect(
    screen.getByRole("button", { name: "Add to library" }),
  ).toBeEnabled();
  fireEvent.click(screen.getByRole("button", { name: "Add to library" }));
  await waitFor(() =>
    expect(toast.success).toHaveBeenCalledExactlyOnceWith(
      "Library update started in the background.",
    ),
  );
  expect(
    screen.queryByText("Library update started in the background."),
  ).not.toBeInTheDocument();
  expect(libraryApi.sync).toHaveBeenCalledExactlyOnceWith("fixture-root");
  expect(screen.queryByText("Scan complete")).not.toBeInTheDocument();
});

it("separates completed local indexing from missing metadata and offers setup for the saved directory", async () => {
  vi.mocked(libraryApi.roots).mockResolvedValue([
    {
      id: "fixture-root",
      name: "Films",
      path: "/media/Movies",
      path_style: "posix",
      kind: "movie",
      metadata_providers: ["tmdb"],
      enabled: true,
      exclude_patterns: [],
      created_at: time,
      updated_at: time,
      last_scan_at: time,
    },
  ]);
  vi.mocked(libraryApi.tasks).mockResolvedValue([
    {
      ...scanTask,
      mode: "automatic",
      phase: "complete",
      state: "waiting_for_user",
      imported_count: 2,
      matched_count: 0,
      review_count: 2,
      unconfigured_count: 2,
      metadata_failed_count: 0,
    },
  ]);
  show();
  expect(await screen.findByText("Local media indexed")).toBeInTheDocument();
  expect(screen.getByText("Metadata not fetched")).toBeInTheDocument();
  expect(
    screen.getByRole("link", { name: "Configure metadata sources" }),
  ).toHaveAttribute("href", "/settings");
  expect(libraryApi.saveRoot).not.toHaveBeenCalled();
  expect(libraryApi.sync).not.toHaveBeenCalled();
});

it.each([
  ["cancelled", "Cancelled"],
  ["interrupted", "Interrupted"],
  ["failed", "Failed"],
] as const)(
  "keeps the %s update outcome visible after local indexing",
  async (state, label) => {
    vi.mocked(libraryApi.roots).mockResolvedValue([
      {
        id: "fixture-root",
        name: "Films",
        path: "/media/Movies",
        path_style: "posix",
        kind: "movie",
        enabled: true,
        exclude_patterns: [],
        created_at: time,
        updated_at: time,
        last_scan_at: time,
      },
    ]);
    vi.mocked(libraryApi.tasks).mockResolvedValue([
      {
        ...scanTask,
        mode: "automatic",
        phase: "metadata",
        state,
        imported_count: 2,
        matched_count: 0,
        review_count: 0,
        unconfigured_count: 0,
        metadata_failed_count: 0,
      },
    ]);
    show();
    expect(await screen.findByText("Local media indexed")).toBeInTheDocument();
    expect(screen.getByText(label)).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "View results" })).toBeInTheDocument();
  },
);

it("accepts one configured catalog source and keeps the other optional", async () => {
  scope.visible = true;
  vi.mocked(adultApi.providers).mockResolvedValue({
    schema_version: 1,
    providers: [source("wikidata")],
    tasks: [],
    requests: [],
  });
  show({ initialAdd: true, initialKind: "adult_video" });
  expect(
    await screen.findByRole("checkbox", { name: "WIKIDATA" }),
  ).toBeChecked();
  expect(providerAuthApi.status).not.toHaveBeenCalled();
  expect(providerApi.snapshot).not.toHaveBeenCalled();
});

it("refreshes an existing directory directly without saving or asking for manual import", async () => {
  vi.mocked(libraryApi.roots).mockResolvedValue([
    {
      id: "fixture-root",
      name: "Films",
      path: "/media/Movies",
      path_style: "posix",
      kind: "movie",
      enabled: true,
      exclude_patterns: [],
      created_at: time,
      updated_at: time,
      last_scan_at: null,
    },
  ]);
  show();
  fireEvent.click(
    await screen.findByRole("button", { name: "Update library" }),
  );
  await waitFor(() =>
    expect(libraryApi.sync).toHaveBeenCalledExactlyOnceWith("fixture-root"),
  );
  expect(libraryApi.saveRoot).not.toHaveBeenCalled();
  expect(libraryApi.confirm).not.toHaveBeenCalled();
});

it("checks credentials for a source that requires them without claiming readiness", async () => {
  vi.mocked(providerApi.snapshot).mockResolvedValue({
    schema_version: 1,
    providers: [source("tmdb", true)],
    tasks: [],
    requests: [],
  });
  vi.mocked(providerAuthApi.status).mockResolvedValue({
    provider_id: "tmdb",
    supported_methods: ["bearer"],
    method: "bearer",
    storage: "system",
    configured: false,
    status: "not_configured",
  });
  show({ initialAdd: true });
  expect(await screen.findByText("Credentials needed")).toBeInTheDocument();
  expect(providerAuthApi.status).toHaveBeenCalledWith("tmdb");
  expect(
    screen.queryByText("Metadata sources configured"),
  ).not.toBeInTheDocument();
});

it("describes automatic music as local tags without checking unrelated online sources", async () => {
  show({ initialAdd: true, initialKind: "music_release" });
  expect(
    await screen.findByText(
      "Music reads local tags first. Continue in media tools when you need online matching.",
    ),
  ).toBeInTheDocument();
  expect(providerApi.snapshot).not.toHaveBeenCalled();
  expect(providerAuthApi.status).not.toHaveBeenCalled();
  expect(
    screen.queryByText("Metadata sources configured"),
  ).not.toBeInTheDocument();
});
