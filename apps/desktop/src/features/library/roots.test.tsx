import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import type { ComponentProps } from "react";
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
import i18n from "../../i18n";
import { PrivacyProvider } from "./context";
import { RootsPage } from "./pages";

vi.mock("../../api/directories", () => ({ pickDirectory: vi.fn() }));
vi.mock("./context", async (original) => ({
  ...(await original<typeof import("./context")>()),
  usePrivacy: () => ({ includeAdult: true }),
}));
vi.mock("../../api/adult", async (original) => ({
  ...(await original<typeof import("../../api/adult")>()),
  adultApi: { roots: vi.fn() },
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
      saveRoot: vi.fn(),
      start: vi.fn(),
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
    <I18nextProvider i18n={i18n}>
      <QueryClientProvider client={client}>
        <PrivacyProvider>
          <RootsPage {...props} />
        </PrivacyProvider>
      </QueryClientProvider>
    </I18nextProvider>,
  );
}
beforeEach(async () => {
  localStorage.clear();
  await i18n.changeLanguage("en-US");
  vi.clearAllMocks();
  vi.mocked(libraryApi.roots).mockResolvedValue([]);
  vi.mocked(adultApi.roots).mockResolvedValue([]);
  vi.mocked(pickDirectory).mockResolvedValue(null);
  vi.mocked(libraryApi.start).mockResolvedValue(scanTask);
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
  fireEvent.click(screen.getByRole("button", { name: "Save" }));
  await waitFor(() =>
    expect(libraryApi.saveRoot).toHaveBeenCalledWith({
      id: null,
      name: "Fixture root",
      path: "/fixture/media",
      kind: "movie",
      enabled: true,
      exclude_patterns: ["**/sample/**", "**/*.tmp"],
    }),
  );
});
it("uses localized validation instead of submitting empty root fields", async () => {
  show();
  await screen.findByText("No library roots yet");
  fireEvent.click(screen.getByRole("button", { name: "Add root" }));
  fireEvent.click(screen.getByRole("button", { name: "Save" }));
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
    fireEvent.click(screen.getByRole("button", { name: "Choose folder" }));
    await waitFor(() =>
      expect(screen.getByLabelText("Root name")).toHaveValue(name),
    );
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() =>
      expect(libraryApi.saveRoot).toHaveBeenCalledWith(
        expect.objectContaining({ path, name, kind: "movie" }),
      ),
    );
    expect(libraryApi.start).not.toHaveBeenCalled();
    expect(await screen.findByRole("status")).toHaveTextContent("Root saved");
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
  fireEvent.click(screen.getByRole("button", { name: "Choose folder" }));
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
  fireEvent.click(screen.getByRole("button", { name: "Choose folder" }));
  expect(screen.getByRole("button", { name: "Save" })).toBeDisabled();
  expect(screen.getByRole("button", { name: "Save and scan" })).toBeDisabled();
  fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
  fireEvent.click(screen.getByRole("button", { name: "Add root" }));
  fireEvent.click(screen.getByRole("button", { name: "Choose folder" }));
  await screen.findByText("/media/Current");
  await act(async () => oldPicker.resolve("/media/Stale"));
  expect(screen.getByLabelText("Root name")).toHaveValue("Current");
  expect(screen.queryByText("/media/Stale")).not.toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Save" })).toBeEnabled();
});

it("keeps name edits made while a folder picker is pending", async () => {
  const picker = deferred<string | null>();
  vi.mocked(pickDirectory).mockReturnValue(picker.promise);
  show();
  fireEvent.click(screen.getByRole("button", { name: "Add root" }));
  fireEvent.click(screen.getByRole("button", { name: "Choose folder" }));
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
  fireEvent.click(screen.getByRole("button", { name: "Choose folder" }));
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
  fireEvent.click(screen.getByRole("button", { name: "Choose folder" }));
  await screen.findByText("/media/Movies");
  const submit = screen.getByRole("button", { name: "Save and scan" });
  fireEvent.click(submit);
  fireEvent.click(submit);
  await waitFor(() => expect(libraryApi.saveRoot).toHaveBeenCalledTimes(1));
  expect(libraryApi.start).not.toHaveBeenCalled();
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
  expect(libraryApi.start).toHaveBeenCalledExactlyOnceWith("fixture-root");
  expect(libraryApi.confirm).not.toHaveBeenCalled();
});

it("retains a successfully saved root when scanning fails and retries only scanning", async () => {
  vi.mocked(pickDirectory).mockResolvedValue("/media/Movies");
  vi.mocked(libraryApi.start)
    .mockRejectedValueOnce(new DesktopError("request_failed"))
    .mockResolvedValueOnce(scanTask);
  const onNavigateScan = vi.fn();
  show({ onNavigateScan });
  fireEvent.click(screen.getByRole("button", { name: "Add root" }));
  fireEvent.click(screen.getByRole("button", { name: "Choose folder" }));
  await screen.findByText("/media/Movies");
  fireEvent.click(screen.getByRole("button", { name: "Save and scan" }));
  expect(
    await screen.findByText(
      "The directory is saved, but the scan could not start.",
    ),
  ).toBeInTheDocument();
  expect(libraryApi.saveRoot).toHaveBeenCalledTimes(1);
  expect(onNavigateScan).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Retry scan" }));
  await waitFor(() =>
    expect(onNavigateScan).toHaveBeenCalledWith({
      root: "fixture-root",
      task: scanTask.id,
    }),
  );
  expect(libraryApi.saveRoot).toHaveBeenCalledTimes(1);
  expect(libraryApi.start).toHaveBeenCalledTimes(2);
});

it("keeps adult save-and-scan navigation inside the existing adult callback", async () => {
  vi.mocked(pickDirectory).mockResolvedValue("/media/Private");
  const onScan = vi.fn();
  const onNavigateScan = vi.fn();
  show({ adult: true, onScan, onNavigateScan });
  fireEvent.click(screen.getByRole("button", { name: "Add root" }));
  fireEvent.click(screen.getByRole("button", { name: "Choose folder" }));
  await screen.findByText("/media/Private");
  fireEvent.click(screen.getByRole("button", { name: "Save and scan" }));
  await waitFor(() =>
    expect(onScan).toHaveBeenCalledWith("fixture-root", scanTask.id),
  );
  expect(onNavigateScan).not.toHaveBeenCalled();
  expect(libraryApi.saveRoot).toHaveBeenCalledWith(
    expect.objectContaining({ kind: "adult_video" }),
  );
  expect(
    screen.queryByRole("link", { name: "View scan" }),
  ).not.toBeInTheDocument();
});
