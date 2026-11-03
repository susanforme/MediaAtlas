import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { type ComponentProps, useState } from "react";
import { I18nextProvider } from "react-i18next";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { adultApi } from "../../api/adult";
import { DesktopError } from "../../api/desktop";
import {
  libraryApi,
  type RootView,
  type ScanCandidate,
  type ScanTask,
} from "../../api/library";
import i18n from "../../i18n";
import { PrivacyProvider } from "./context";
import { ScannerPage } from "./scanner";
import type { ScannerSearch } from "./scanner-navigation";

vi.mock("@tanstack/react-router", () => ({
  Link: ({ to, children }: { to: string; children: React.ReactNode }) => (
    <a href={to}>{children}</a>
  ),
}));
vi.mock("./context", async (original) => ({
  ...(await original<typeof import("./context")>()),
  usePrivacy: () => ({ includeAdult: true }),
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
const candidate: ScanCandidate = {
  id: "fixture-candidate",
  root_id: root.id,
  relative_path: "Fixture/movie.mkv",
  size_bytes: 123,
  modified_at: time,
  kind: "movie",
  title: "Fixture movie",
  year: 2026,
  part: null,
  series_title: null,
  season_number: null,
  episode_number: null,
  absolute_number: null,
  catalog_number: null,
  needs_review: true,
  warnings: ["fixture_warning"],
  existing_media_id: null,
  status: "pending",
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
    localStorage.clear();
    await i18n.changeLanguage("en-US");
    vi.mocked(libraryApi.roots).mockResolvedValue([root]);
    vi.mocked(libraryApi.tasks).mockResolvedValue([task]);
    vi.mocked(adultApi.roots).mockResolvedValue([
      { ...root, kind: "adult_video" },
    ]);
    vi.mocked(adultApi.tasks).mockResolvedValue([task]);
    vi.mocked(libraryApi.start).mockResolvedValue({
      ...task,
      id: "fixture-new-scan",
      state: "queued",
    });
    vi.mocked(libraryApi.preview).mockResolvedValue({
      task_id: task.id,
      items: [candidate],
      next_cursor: "fixture-next-page",
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

  it("requires selection and an explicit confirmation before importing uncertain candidates", async () => {
    show();
    await screen.findByText("Fixture movie");
    expect(
      screen.getByRole("button", { name: "Confirm selected import" }),
    ).toBeDisabled();
    expect(libraryApi.confirm).not.toHaveBeenCalled();
    fireEvent.click(
      screen.getByRole("checkbox", {
        name: "Confirm selected import: Fixture movie",
      }),
    );
    fireEvent.click(
      screen.getByRole("button", { name: "Confirm selected import" }),
    );
    expect(
      screen.getByRole("dialog", { name: "Confirm import preview" }),
    ).toBeInTheDocument();
    expect(libraryApi.confirm).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Import selected" }));
    await waitFor(() =>
      expect(libraryApi.confirm).toHaveBeenCalledWith(task.id, [candidate.id]),
    );
    expect(
      await screen.findByText("Imported 1; 0 unchanged."),
    ).toBeInTheDocument();
  });

  it("cancels the task without importing candidates", async () => {
    show();
    await screen.findByText("Fixture movie");
    fireEvent.click(screen.getByRole("button", { name: "Cancel task" }));
    await waitFor(() =>
      expect(libraryApi.cancel).toHaveBeenCalledWith(task.id),
    );
    expect(libraryApi.confirm).not.toHaveBeenCalled();
  });

  it("requests a bounded next page with privacy filtering", async () => {
    show();
    await screen.findByText("Fixture movie");
    fireEvent.click(screen.getByRole("button", { name: "Next" }));
    await waitFor(() =>
      expect(libraryApi.preview).toHaveBeenLastCalledWith(
        task.id,
        "fixture-next-page",
        false,
      ),
    );
    expect(libraryApi.roots).toHaveBeenCalledWith(false);
    expect(libraryApi.tasks).toHaveBeenCalledWith(false);
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

  it("preselects the only enabled directory while keeping scan startup explicit", async () => {
    vi.mocked(libraryApi.tasks).mockResolvedValue([]);
    const view = show({});
    await waitFor(() =>
      expect(screen.getByLabelText("Choose an enabled root")).toHaveValue(
        root.id,
      ),
    );
    expect(screen.getByRole("button", { name: "Start scan" })).toBeEnabled();
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
    expect(await screen.findByText("Fixture movie")).toBeInTheDocument();
    expect(screen.getByLabelText("Recent scans")).toHaveValue(task.id);
    expect(view.onSearch).toHaveBeenCalledWith({
      root: root.id,
      task: task.id,
    });
    const options = within(screen.getByLabelText("Recent scans")).getAllByRole(
      "option",
    );
    expect(options[0]).toHaveTextContent(root.name);
    expect(options[0]).not.toHaveTextContent(task.id);
    expect(options[1]).not.toHaveTextContent("opaque-old-id");
  });

  it("can retry a failed scan startup and blocks duplicate requests while one is pending", async () => {
    vi.mocked(libraryApi.tasks).mockResolvedValue([]);
    vi.mocked(libraryApi.start).mockRejectedValueOnce(
      new DesktopError("request_failed"),
    );
    const pending = deferred<ScanTask>();
    vi.mocked(libraryApi.start).mockReturnValueOnce(pending.promise);
    const view = show({ root: root.id });
    const startButton = await screen.findByRole("button", {
      name: "Start scan",
    });
    await waitFor(() => expect(startButton).toBeEnabled());
    fireEvent.click(startButton);
    await screen.findByRole("alert");
    const retryButton = screen.getByRole("button", { name: "Start scan" });
    fireEvent.click(retryButton);
    fireEvent.click(retryButton);
    await waitFor(() =>
      expect(screen.getByLabelText("Choose an enabled root")).toBeDisabled(),
    );
    await waitFor(() => expect(libraryApi.start).toHaveBeenCalledTimes(2));
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
        "Scanning. Review and confirm candidates when it finishes.",
      ),
    ).toBeInTheDocument();
    expect(
      screen.queryByText("This scan has no candidates awaiting confirmation."),
    ).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Start scan" })).toBeDisabled();
  });

  it("keeps confirmation errors inside the dialog and retries the selected import explicitly", async () => {
    vi.mocked(libraryApi.confirm)
      .mockRejectedValueOnce(new DesktopError("request_failed"))
      .mockResolvedValueOnce({ imported: 1, unchanged: 0, needs_review: 0 });
    show();
    await screen.findByText("Fixture movie");
    fireEvent.click(
      screen.getByRole("checkbox", {
        name: "Confirm selected import: Fixture movie",
      }),
    );
    fireEvent.click(
      screen.getByRole("button", { name: "Confirm selected import" }),
    );
    fireEvent.click(screen.getByRole("button", { name: "Import selected" }));
    const dialog = screen.getByRole("dialog", {
      name: "Confirm import preview",
    });
    expect(await within(dialog).findByRole("alert")).toBeInTheDocument();
    expect(libraryApi.confirm).toHaveBeenCalledTimes(1);
    fireEvent.click(
      within(dialog).getByRole("button", { name: "Import selected" }),
    );
    await screen.findByText("Imported 1; 0 unchanged.");
    expect(libraryApi.confirm).toHaveBeenCalledTimes(2);
    expect(
      screen.getByRole("link", { name: "Browse imported media" }),
    ).toHaveAttribute("href", "/library");
    expect(
      screen.getByRole("link", { name: "Open media tools to scrape" }),
    ).toHaveAttribute("href", "/tools");
  });

  it("clears selected candidates when switching tasks", async () => {
    const other = {
      ...task,
      id: "second-task",
      created_at: "2026-11-02T00:00:00Z",
    };
    vi.mocked(libraryApi.tasks).mockResolvedValue([task, other]);
    vi.mocked(libraryApi.preview).mockImplementation(async (id) => ({
      task_id: id,
      items: [
        { ...candidate, id: id === task.id ? candidate.id : "other-candidate" },
      ],
      next_cursor: null,
    }));
    show();
    await screen.findByText("Fixture movie");
    fireEvent.click(
      screen.getByRole("checkbox", {
        name: "Confirm selected import: Fixture movie",
      }),
    );
    expect(
      screen.getByRole("button", { name: "Confirm selected import" }),
    ).toBeEnabled();
    fireEvent.change(screen.getByLabelText("Recent scans"), {
      target: { value: other.id },
    });
    await waitFor(() =>
      expect(libraryApi.preview).toHaveBeenCalledWith(other.id, null, false),
    );
    expect(
      await screen.findByRole("button", { name: "Confirm selected import" }),
    ).toBeDisabled();
    expect(
      screen.getByRole("checkbox", {
        name: "Confirm selected import: Fixture movie",
      }),
    ).not.toBeChecked();
    expect(libraryApi.confirm).not.toHaveBeenCalled();
  });

  it("keeps adult empty-state and completion navigation inside supplied adult callbacks", async () => {
    vi.mocked(adultApi.roots).mockResolvedValue([]);
    vi.mocked(adultApi.tasks).mockResolvedValue([]);
    const onAddRoot = vi.fn();
    const empty = show({}, { adult: true, onAddRoot });
    fireEvent.click(
      await screen.findByRole("button", { name: "Add media directory" }),
    );
    expect(onAddRoot).toHaveBeenCalledOnce();
    expect(screen.queryByRole("link")).not.toBeInTheDocument();
    empty.unmount();
    vi.mocked(adultApi.roots).mockResolvedValue([
      { ...root, kind: "adult_video" },
    ]);
    vi.mocked(adultApi.tasks).mockResolvedValue([
      { ...task, state: "succeeded", imported_count: 1 },
    ]);
    const onBrowse = vi.fn();
    show({ root: root.id, task: task.id }, { adult: true, onBrowse });
    fireEvent.click(
      await screen.findByRole("button", { name: "Browse imported media" }),
    );
    expect(onBrowse).toHaveBeenCalledOnce();
    expect(libraryApi.roots).not.toHaveBeenCalled();
    expect(libraryApi.tasks).not.toHaveBeenCalled();
    expect(screen.queryByRole("link")).not.toBeInTheDocument();
  });
});
