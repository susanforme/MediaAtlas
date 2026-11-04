import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { type ComponentProps, useState } from "react";
import { I18nextProvider } from "react-i18next";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { adultApi } from "../../api/adult";
import { DesktopError } from "../../api/desktop";
import { libraryApi, type RootView, type ScanTask } from "../../api/library";
import i18n from "../../i18n";
import { selectOptions } from "../../test/select";
import { PrivacyProvider } from "./context";
import { ScannerPage, TasksPage } from "./scanner";
import type { ScannerSearch } from "./scanner-navigation";

vi.mock("../providers/pages", () => ({ ProviderTasks: () => null }));

const privacy = vi.hoisted(() => ({ visible: false, allowed: true }));
vi.mock("@tanstack/react-router", () => ({
  Link: ({
    to,
    children,
    search,
  }: {
    to: string;
    children: React.ReactNode;
    search?: unknown;
  }) => (
    <a href={to} data-search={JSON.stringify(search)}>
      {children}
    </a>
  ),
}));
vi.mock("./context", async (original) => ({
  ...(await original<typeof import("./context")>()),
  usePrivacy: () => ({
    includeAdult: privacy.allowed,
    settings: { entry_visible: privacy.visible },
  }),
}));
vi.mock("../../api/adult", async (original) => ({
  ...(await original<typeof import("../../api/adult")>()),
  adultApi: { roots: vi.fn(), tasks: vi.fn() },
}));
vi.mock("../../api/library", async (original) => {
  const module = await original<typeof import("../../api/library")>();
  return {
    ...module,
    libraryApi: {
      ...module.libraryApi,
      roots: vi.fn(),
      tasks: vi.fn(),
      preview: vi.fn(),
      confirm: vi.fn(),
      cancel: vi.fn(),
      start: vi.fn(),
      sync: vi.fn(),
    },
  };
});
const time = "2026-11-03T00:00:00Z";
const task: ScanTask = {
  id: "fixture-task",
  root_id: "fixture-root",
  state: "waiting_for_user",
  visited_files: 2,
  skipped_files: 0,
  candidate_count: 2,
  imported_count: 0,
  warnings: 1,
  error_code: null,
  created_at: time,
  updated_at: time,
};
const root: RootView = {
  id: "fixture-root",
  name: "Fixture root",
  path: "/fixture",
  path_style: "posix",
  kind: "movie",
  enabled: true,
  exclude_patterns: [],
  last_scan_at: null,
  created_at: time,
  updated_at: time,
};
function ControlledScanner({
  initialSearch,
  onSearch,
  ...props
}: Omit<ComponentProps<typeof ScannerPage>, "search" | "onSearch"> & {
  initialSearch: ScannerSearch;
  onSearch: (value: ScannerSearch) => void;
}) {
  const [search, setSearch] = useState(initialSearch);
  return (
    <ScannerPage
      {...props}
      search={search}
      onSearch={(next) => {
        onSearch(next);
        setSearch(next);
      }}
    />
  );
}
function show(
  initialSearch: ScannerSearch = { root: root.id, task: task.id },
  props: Omit<ComponentProps<typeof ScannerPage>, "search" | "onSearch"> = {},
) {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  const onSearch = vi.fn();
  const view = render(
    <I18nextProvider i18n={i18n}>
      <QueryClientProvider client={client}>
        <PrivacyProvider>
          <ControlledScanner
            {...props}
            initialSearch={initialSearch}
            onSearch={onSearch}
          />
        </PrivacyProvider>
      </QueryClientProvider>
    </I18nextProvider>,
  );
  return { ...view, onSearch };
}
function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}

describe("Fixture scan review", () => {
  beforeEach(async () => {
    vi.clearAllMocks();
    privacy.visible = false;
    privacy.allowed = true;
    localStorage.clear();
    await i18n.changeLanguage("en-US");
    vi.mocked(libraryApi.roots).mockResolvedValue([root]);
    vi.mocked(libraryApi.tasks).mockResolvedValue([task]);
    vi.mocked(adultApi.roots).mockResolvedValue([
      { ...root, kind: "adult_video" },
    ]);
    vi.mocked(adultApi.tasks).mockResolvedValue([task]);
    vi.mocked(libraryApi.sync).mockResolvedValue({
      ...task,
      id: "fixture-new-scan",
      state: "queued",
    });
    vi.mocked(libraryApi.confirm).mockResolvedValue({
      imported: 1,
      unchanged: 0,
      needs_review: 1,
    });
    vi.mocked(libraryApi.cancel).mockResolvedValue({
      ...task,
      state: "cancelled",
    });
  });

  it("cancels the task without importing candidates", async () => {
    vi.mocked(libraryApi.tasks).mockResolvedValue([
      { ...task, state: "running" },
    ]);
    show();
    await screen.findByRole("button", { name: "Cancel task" });
    fireEvent.click(screen.getByRole("button", { name: "Cancel task" }));
    await waitFor(() =>
      expect(libraryApi.cancel).toHaveBeenCalledWith(task.id),
    );
    expect(libraryApi.confirm).not.toHaveBeenCalled();
  });

  it("guides an empty scanner to adding a directory without starting a task", async () => {
    vi.mocked(libraryApi.roots).mockResolvedValue([]);
    vi.mocked(libraryApi.tasks).mockResolvedValue([]);
    show({});
    expect(
      await screen.findByRole("link", { name: "Add media directory" }),
    ).toHaveAttribute("href", "/roots");
    expect(screen.queryByLabelText("Recent scans")).not.toBeInTheDocument();
    expect(libraryApi.start).not.toHaveBeenCalled();
  });

  it("preselects the only enabled directory while keeping an extra refresh explicit", async () => {
    vi.mocked(libraryApi.tasks).mockResolvedValue([]);
    const view = show({});
    await waitFor(() =>
      expect(screen.getByLabelText("Choose an enabled root")).toHaveValue(
        root.id,
      ),
    );
    expect(
      screen.getByRole("button", { name: "Update library" }),
    ).toBeEnabled();
    expect(libraryApi.start).not.toHaveBeenCalled();
    expect(view.onSearch).toHaveBeenCalledWith({
      root: root.id,
      task: undefined,
    });
  });

  it("selects the latest scan of the directory using its name and time rather than an opaque task ID", async () => {
    vi.mocked(libraryApi.tasks).mockResolvedValue([
      { ...task, id: "opaque-old-id", created_at: "2026-01-01T00:00:00Z" },
      task,
    ]);
    const view = show({});
    await screen.findByText(
      "Update the library to process this directory automatically, without confirming individual imports.",
    );
    expect(screen.getByLabelText("Recent scans")).toHaveValue(task.id);
    expect(view.onSearch).toHaveBeenCalledWith({
      root: root.id,
      task: task.id,
    });
    const options = selectOptions(
      screen.getByRole("combobox", { name: "Recent scans" }),
    );
    expect(options[0]).toHaveTextContent(root.name);
    expect(options[0]).not.toHaveTextContent(task.id);
    expect(options[1]).not.toHaveTextContent("opaque-old-id");
  });

  it("can retry a failed scan startup and blocks duplicate requests while one is pending", async () => {
    vi.mocked(libraryApi.tasks).mockResolvedValue([]);
    vi.mocked(libraryApi.sync).mockRejectedValueOnce(
      new DesktopError("request_failed"),
    );
    const pending = deferred<ScanTask>();
    vi.mocked(libraryApi.sync).mockReturnValueOnce(pending.promise);
    const view = show({ root: root.id });
    const startButton = await screen.findByRole("button", {
      name: "Update library",
    });
    await waitFor(() => expect(startButton).toBeEnabled());
    fireEvent.click(startButton);
    await screen.findByRole("alert");
    const retryButton = screen.getByRole("button", { name: "Update library" });
    fireEvent.click(retryButton);
    fireEvent.click(retryButton);
    await waitFor(() =>
      expect(screen.getByLabelText("Choose an enabled root")).toBeDisabled(),
    );
    await waitFor(() => expect(libraryApi.sync).toHaveBeenCalledTimes(2));
    await act(async () =>
      pending.resolve({ ...task, id: "new-task", state: "queued" }),
    );
    expect(view.onSearch).toHaveBeenCalledWith({
      root: root.id,
      task: "new-task",
    });
  });

  it("shows running status instead of an empty result while a scan is in progress", async () => {
    vi.mocked(libraryApi.tasks).mockResolvedValue([
      { ...task, state: "running" },
    ]);
    vi.mocked(libraryApi.preview).mockResolvedValue({
      task_id: task.id,
      items: [],
      next_cursor: null,
    });
    show();
    expect(
      await screen.findByText(
        "This directory scan is running. Update the library after it finishes to process its results automatically.",
      ),
    ).toBeInTheDocument();
    expect(
      screen.queryByText("This scan has no candidates awaiting confirmation."),
    ).not.toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Update library" }),
    ).toBeDisabled();
  });

  it("shows an automatic update as background progress without candidate import controls", async () => {
    vi.mocked(libraryApi.tasks).mockResolvedValue([
      {
        ...task,
        mode: "automatic",
        phase: "metadata",
        state: "running",
        imported_count: 2,
        matched_count: 1,
        review_count: 0,
        metadata_failed_count: 0,
      },
    ]);
    show();
    expect(await screen.findByText("Fetching metadata")).toBeInTheDocument();
    expect(
      screen.getByText(
        "Updating in the background. Discovered media is available to browse.",
      ),
    ).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "View library" })).toHaveAttribute(
      "href",
      "/library",
    );
    expect(screen.queryByRole("checkbox")).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Confirm selected import" }),
    ).not.toBeInTheDocument();
    expect(libraryApi.preview).not.toHaveBeenCalled();
    expect(libraryApi.confirm).not.toHaveBeenCalled();
  });

  it("leaves local media available when automatic matching needs attention and allows a fresh update", async () => {
    vi.mocked(libraryApi.tasks).mockResolvedValue([
      {
        ...task,
        mode: "automatic",
        phase: "complete",
        state: "waiting_for_user",
        imported_count: 2,
        matched_count: 0,
        review_count: 2,
        unconfigured_count: 1,
        metadata_failed_count: 1,
      },
    ]);
    show();
    expect(
      (await screen.findAllByText("Metadata source setup required"))[0],
    ).toBeInTheDocument();
    expect(
      screen.getByRole("link", { name: "Review unmatched media" }),
    ).toHaveAttribute("href", "/tools");
    expect(
      screen.getByRole("link", { name: "Configure source and continue" }),
    ).toHaveAttribute("href", "/settings");
    expect(
      screen.getByRole("link", { name: "Configure source and continue" }),
    ).toHaveAttribute(
      "data-search",
      JSON.stringify({
        from: "library",
        root: root.id,
        return: { to: "library", search: { root: root.id } },
      }),
    );
    expect(
      screen.getByRole("button", { name: "Update library" }),
    ).toBeEnabled();
    expect(
      screen.queryByRole("button", { name: "Cancel task" }),
    ).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Update library" }));
    await waitFor(() =>
      expect(libraryApi.sync).toHaveBeenCalledExactlyOnceWith(root.id),
    );
    expect(libraryApi.confirm).not.toHaveBeenCalled();
  });
  it("merges authorized adult directories into the same selector and updates them without an import step", async () => {
    privacy.visible = true;
    const privateRoot = {
      ...root,
      id: "private-root",
      name: "Private directory",
      kind: "adult_video" as const,
    };
    const privateTask = {
      ...task,
      id: "private-task",
      root_id: privateRoot.id,
      mode: "automatic" as const,
      phase: "complete" as const,
      review_count: 1,
      matched_count: 0,
      metadata_failed_count: 0,
    };
    vi.mocked(adultApi.roots).mockResolvedValue([privateRoot]);
    vi.mocked(adultApi.tasks).mockResolvedValue([privateTask]);
    vi.mocked(libraryApi.sync).mockResolvedValue({
      ...privateTask,
      id: "new-private-task",
      state: "queued",
    });
    const view = show({
      root: privateRoot.id,
      task: privateTask.id,
      kind: "adult_video",
    });
    selectOptions(await screen.findByLabelText("Choose an enabled root"));
    expect(
      await screen.findByRole("option", { name: "Private directory" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("option", { name: "Fixture root" }),
    ).toBeInTheDocument();
    expect(screen.queryByRole("checkbox")).not.toBeInTheDocument();
    expect(libraryApi.preview).not.toHaveBeenCalled();
    expect(
      screen.getByRole("link", { name: "Review unmatched media" }),
    ).toHaveAttribute(
      "data-search",
      JSON.stringify({
        root: privateRoot.id,
        health: "metadata_needs_review",
        kind: "adult_video",
      }),
    );
    expect(screen.getByRole("link", { name: "View library" })).toHaveAttribute(
      "data-search",
      JSON.stringify({ root: privateRoot.id, kind: "adult_video" }),
    );
    fireEvent.click(screen.getByRole("button", { name: "Update library" }));
    await waitFor(() =>
      expect(libraryApi.sync).toHaveBeenCalledExactlyOnceWith(privateRoot.id),
    );
    expect(view.onSearch).toHaveBeenCalledWith({
      root: privateRoot.id,
      task: "new-private-task",
      kind: "adult_video",
    });
    expect(libraryApi.confirm).not.toHaveBeenCalled();
    expect(
      screen.queryByRole("link", { name: "Configure credentials" }),
    ).not.toBeInTheDocument();
  });

  it("does not query private tasks or roots without the active backend grant", async () => {
    privacy.visible = true;
    privacy.allowed = false;
    show();
    await screen.findByText(
      "Update the library to process this directory automatically, without confirming individual imports.",
    );
    expect(adultApi.roots).not.toHaveBeenCalled();
    expect(adultApi.tasks).not.toHaveBeenCalled();
    expect(libraryApi.preview).not.toHaveBeenCalled();
    expect(libraryApi.confirm).not.toHaveBeenCalled();
  });
  it.each(["invalid_root", "invalid_exclude_pattern", "local_file_unreadable"])(
    "offers directory management for %s without exposing a raw error",
    async (error_code) => {
      vi.mocked(libraryApi.tasks).mockResolvedValue([
        { ...task, state: "failed", error_code },
      ]);
      show();
      expect(
        await screen.findByRole("link", { name: "Manage directories" }),
      ).toHaveAttribute("href", "/roots");
      expect(screen.getByRole("alert")).not.toHaveTextContent(error_code);
      expect(libraryApi.sync).not.toHaveBeenCalled();
    },
  );

  it("restarts a stopped directory update explicitly while preserving indexed content", async () => {
    vi.mocked(libraryApi.tasks).mockResolvedValue([
      { ...task, mode: "automatic", state: "cancelled", imported_count: 2 },
    ]);
    show();
    expect(
      await screen.findByText(
        "The update stopped. Already indexed content and saved metadata are retained. Update the directory again to process unfinished items.",
      ),
    ).toBeInTheDocument();
    fireEvent.click(
      screen.getByRole("button", { name: "Update directory again" }),
    );
    await waitFor(() =>
      expect(libraryApi.sync).toHaveBeenCalledExactlyOnceWith(root.id),
    );
    expect(libraryApi.confirm).not.toHaveBeenCalled();
  });

  it("shows safe recovery and preserves adult scope without exposing arbitrary error details", async () => {
    privacy.visible = true;
    const error_code = "private/path/token=secret";
    vi.mocked(libraryApi.tasks).mockResolvedValue([]);
    vi.mocked(adultApi.tasks).mockResolvedValue([
      { ...task, mode: "automatic", state: "failed", error_code },
    ]);
    show({ root: root.id, task: task.id, kind: "adult_video" });
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "The directory update did not finish.",
    );
    expect(document.body).not.toHaveTextContent(error_code);
    fireEvent.click(
      screen.getByRole("button", { name: "Update directory again" }),
    );
    await waitFor(() =>
      expect(libraryApi.sync).toHaveBeenCalledExactlyOnceWith(root.id),
    );
  });

  it("uses the same recovery instructions and real retry destination in task history", async () => {
    vi.mocked(libraryApi.tasks).mockResolvedValue([
      { ...task, state: "failed", error_code: "files_changed" },
    ]);
    const client = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });
    render(
      <I18nextProvider i18n={i18n}>
        <QueryClientProvider client={client}>
          <TasksPage />
        </QueryClientProvider>
      </I18nextProvider>,
    );
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Some files changed during the scan and have not been added.",
    );
    const retry = screen.getByRole("link", { name: "View and update again" });
    expect(retry).toHaveAttribute("href", "/scanner");
    expect(retry).toHaveAttribute(
      "data-search",
      JSON.stringify({ root: root.id, task: task.id }),
    );
    expect(libraryApi.sync).not.toHaveBeenCalled();
  });
});
