import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import type { ReactNode } from "react";
import { I18nextProvider } from "react-i18next";
import { beforeEach, expect, it, vi } from "vitest";
import { adultApi } from "../../api/adult";
import { DesktopError } from "../../api/desktop";
import { libraryApi, type RootView, type ScanTask } from "../../api/library";
import { providerApi } from "../../api/providers";
import i18n from "../../i18n";
import { fixtureSnapshot, fixtureTask } from "../providers/fixtures";
import { TasksPage } from "./tasks";

const privacy = vi.hoisted(() => ({ visible: false, allowed: false }));
vi.mock("./context", () => ({
  usePrivacy: () => ({
    includeAdult: privacy.allowed,
    settings: { entry_visible: privacy.visible },
  }),
}));
vi.mock("@tanstack/react-router", () => ({
  Link: ({
    to,
    search,
    children,
  }: {
    to: string;
    search?: unknown;
    children: ReactNode;
  }) => (
    <a href={to} data-search={JSON.stringify(search)}>{children}</a>
  ),
}));
vi.mock("../../api/library", async (original) => ({
  ...(await original<typeof import("../../api/library")>()),
  libraryApi: { roots: vi.fn(), tasks: vi.fn(), cancel: vi.fn(), sync: vi.fn() },
}));
vi.mock("../../api/adult", async (original) => ({
  ...(await original<typeof import("../../api/adult")>()),
  adultApi: { roots: vi.fn(), tasks: vi.fn() },
}));
vi.mock("../../api/providers", async (original) => ({
  ...(await original<typeof import("../../api/providers")>()),
  providerApi: { snapshot: vi.fn(), cancel: vi.fn() },
}));

const time = "2026-11-03T00:00:00Z";
const root: RootView = {
  id: "movies",
  name: "Movies",
  path: "/fixture",
  path_style: "posix",
  kind: "movie",
  enabled: true,
  exclude_patterns: [],
  last_scan_at: null,
  created_at: time,
  updated_at: time,
};
const task: ScanTask = {
  id: "scan-task",
  root_id: root.id,
  state: "running",
  visited_files: 8,
  skipped_files: 1,
  candidate_count: 7,
  imported_count: 7,
  warnings: 0,
  error_code: null,
  created_at: time,
  updated_at: time,
  mode: "automatic",
  phase: "metadata",
  matched_count: 3,
  review_count: 3,
  unconfigured_count: 2,
  metadata_failed_count: 1,
};

function show(
  client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  }),
) {
  return render(<TasksPage />, {
    wrapper: ({ children }: { children: ReactNode }) => (
      <I18nextProvider i18n={i18n}>
        <QueryClientProvider client={client}>{children}</QueryClientProvider>
      </I18nextProvider>
    ),
  });
}

beforeEach(async () => {
  vi.clearAllMocks();
  privacy.visible = false;
  privacy.allowed = false;
  await i18n.changeLanguage("en-US");
  vi.mocked(libraryApi.roots).mockResolvedValue([root]);
  vi.mocked(libraryApi.tasks).mockResolvedValue([task]);
  vi.mocked(libraryApi.cancel).mockResolvedValue({ ...task, state: "cancelled" });
  vi.mocked(providerApi.snapshot).mockResolvedValue(fixtureSnapshot());
  vi.mocked(providerApi.cancel).mockResolvedValue(true);
  vi.mocked(adultApi.roots).mockResolvedValue([]);
  vi.mocked(adultApi.tasks).mockResolvedValue([]);
});

it("merges task types into one newest-first log with readable content and real outcomes", async () => {
  vi.mocked(providerApi.snapshot).mockResolvedValue({
    ...fixtureSnapshot(),
    tasks: [{ ...fixtureTask, updated_at: "2026-11-02T23:30:00-02:00" }],
  });
  show();
  await screen.findByText("Offline demo · Search for Fixture movie");
  const rows = within(
    screen.getByRole("list", { name: "Task log" }),
  ).getAllByRole("listitem");
  expect(rows).toHaveLength(2);
  expect(rows[0]).toHaveTextContent("Offline demo · Search for Fixture movie");
  expect(rows[1]).toHaveTextContent("Refresh library · Movies");
  expect(rows[1]).toHaveTextContent("Fetching metadata");
  expect(rows[1]).toHaveTextContent("3 items identified");
  expect(rows[1]).toHaveTextContent("1 items to review");
  expect(rows[1]).toHaveTextContent("2 items awaiting source setup");
  expect(rows[1]).toHaveTextContent("1 metadata requests failed");
  expect(screen.queryByText(task.id)).not.toBeInTheDocument();
  expect(screen.queryByText(fixtureTask.id)).not.toBeInTheDocument();
  expect(screen.queryByText("Local scan tasks")).not.toBeInTheDocument();
  expect(libraryApi.tasks).toHaveBeenCalledWith(false);
  expect(adultApi.tasks).not.toHaveBeenCalled();
});

it("cancels each active task through its own API and keeps completed tasks read-only", async () => {
  vi.mocked(providerApi.snapshot).mockResolvedValue({
    ...fixtureSnapshot(),
    tasks: [
      fixtureTask,
      {
        ...fixtureTask,
        id: "done",
        query: "Completed movie",
        state: "succeeded",
        result_summary: "fixture_complete",
      },
    ],
  });
  show();
  await screen.findByText(
    "Offline JSON and HTML combination passed. No media was changed.",
  );
  expect(screen.getAllByRole("button", { name: "Cancel task" })).toHaveLength(2);
  const scanRow = screen.getByText("Refresh library · Movies").closest("li");
  const providerRow = screen
    .getByText("Offline demo · Search for Fixture movie")
    .closest("li");
  if (!scanRow || !providerRow) throw new Error("Task rows are missing");
  fireEvent.click(within(scanRow).getByRole("button", { name: "Cancel task" }));
  await waitFor(() =>
    expect(libraryApi.cancel).toHaveBeenCalledExactlyOnceWith(task.id),
  );
  await waitFor(() =>
    expect(
      within(providerRow).getByRole("button", { name: "Cancel task" }),
    ).toBeEnabled(),
  );
  fireEvent.click(
    within(providerRow).getByRole("button", { name: "Cancel task" }),
  );
  await waitFor(() =>
    expect(providerApi.cancel).toHaveBeenCalledExactlyOnceWith(fixtureTask.id),
  );
});

it("preserves manual scan previews and cancellation while awaiting review", async () => {
  vi.mocked(libraryApi.tasks).mockResolvedValue([
    { ...task, mode: "manual", state: "waiting_for_user" },
  ]);
  show();
  await screen.findByText("Scan folder · Movies");
  expect(screen.getByRole("button", { name: "Cancel task" })).toBeEnabled();
  expect(screen.getByRole("link", { name: "Details" })).toHaveAttribute(
    "data-search",
    JSON.stringify({ root: root.id, task: task.id }),
  );
});

it("shows successful automatic updates with unresolved metadata as needing attention", async () => {
  vi.mocked(libraryApi.tasks).mockResolvedValue([
    { ...task, state: "succeeded", phase: "complete" },
  ]);
  show();
  await screen.findByText("Needs attention");
  expect(screen.getByText(/Metadata source setup required/)).toBeInTheDocument();
  expect(
    screen.queryByRole("button", { name: "Cancel task" }),
  ).not.toBeInTheDocument();
});

it("keeps scan records visible when the provider channel fails", async () => {
  vi.mocked(providerApi.snapshot).mockRejectedValue(
    new DesktopError("desktop_required"),
  );
  show();
  await screen.findByText("Open in the desktop app");
  expect(screen.getByText("Refresh library · Movies")).toBeInTheDocument();
  expect(screen.queryByText("No task records yet")).not.toBeInTheDocument();
});

it("keeps provider records visible while scans are still loading", async () => {
  vi.mocked(libraryApi.tasks).mockReturnValue(new Promise(() => {}));
  vi.mocked(providerApi.snapshot).mockResolvedValue({
    ...fixtureSnapshot(),
    tasks: [fixtureTask],
  });
  show();
  await screen.findByText("Offline demo · Search for Fixture movie");
  expect(screen.getByRole("status")).toBeInTheDocument();
});

it("shows one empty state when every task channel is empty", async () => {
  vi.mocked(libraryApi.tasks).mockResolvedValue([]);
  show();
  await screen.findByText("No task records yet");
  expect(screen.queryByRole("list", { name: "Task log" })).not.toBeInTheDocument();
});

it("preserves safe recovery instructions and the real retry destination", async () => {
  vi.mocked(libraryApi.tasks).mockResolvedValue([
    { ...task, state: "failed", error_code: "files_changed" },
  ]);
  show();
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

it("never renders arbitrary scan error details", async () => {
  vi.mocked(libraryApi.tasks).mockResolvedValue([
    { ...task, state: "failed", error_code: "private/path/token=secret" },
  ]);
  show();
  expect(await screen.findByRole("alert")).toHaveTextContent(
    "The directory update did not finish.",
  );
  expect(document.body).not.toHaveTextContent("private/path/token=secret");
});

it("updates provider history on polling without navigation", async () => {
  vi.mocked(libraryApi.tasks).mockResolvedValue([]);
  vi.mocked(providerApi.snapshot)
    .mockResolvedValueOnce({ ...fixtureSnapshot(), tasks: [fixtureTask] })
    .mockResolvedValue({
      ...fixtureSnapshot(),
      tasks: [
        { ...fixtureTask, state: "succeeded", result_summary: "fixture_complete" },
      ],
    });
  show();
  await screen.findByText("Running");
  await screen.findByText(
    "Offline JSON and HTML combination passed. No media was changed.",
    {},
    { timeout: 3500 },
  );
});

it("localizes provider task content, states and failures in Chinese", async () => {
  await i18n.changeLanguage("zh-CN");
  vi.mocked(libraryApi.tasks).mockResolvedValue([]);
  vi.mocked(providerApi.snapshot).mockResolvedValue({
    ...fixtureSnapshot(),
    tasks: [{ ...fixtureTask, state: "failed", failure: "captcha" }],
  });
  show();
  await screen.findByText("需要验证码，未尝试绕过");
  expect(screen.getByText("失败")).toBeInTheDocument();
  expect(screen.getByText("离线演示 · 检索 Fixture movie")).toBeInTheDocument();
});

it("keeps cached private tasks hidden until both visibility and session authorization are available", async () => {
  privacy.visible = true;
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  client.setQueryData(["adult", "scan-tasks"], [
    { ...task, id: "private-task", root_id: "private-root" },
  ]);
  client.setQueryData(["adult", "roots"], [
    { ...root, id: "private-root", name: "Private folder", kind: "adult_video" },
  ]);
  show(client);
  await screen.findByText("Refresh library · Movies");
  expect(screen.queryByText(/Private folder/)).not.toBeInTheDocument();
  expect(adultApi.tasks).not.toHaveBeenCalled();
  expect(adultApi.roots).not.toHaveBeenCalled();
});

it("preserves private detail scope and hides rows immediately when the feature is disabled", async () => {
  privacy.visible = true;
  privacy.allowed = true;
  vi.mocked(libraryApi.tasks).mockResolvedValue([]);
  vi.mocked(adultApi.roots).mockResolvedValue([
    { ...root, id: "private-root", name: "Private folder", kind: "adult_video" },
  ]);
  vi.mocked(adultApi.tasks).mockResolvedValue([
    { ...task, root_id: "private-root", state: "failed", error_code: "files_changed" },
  ]);
  const view = show();
  await screen.findByText("Refresh library · Private folder");
  for (const name of ["Details", "View and update again"]) {
    expect(screen.getByRole("link", { name })).toHaveAttribute(
      "data-search",
      JSON.stringify({ root: "private-root", task: task.id, kind: "adult_video" }),
    );
  }
  privacy.visible = false;
  view.rerender(<TasksPage />);
  expect(screen.queryByText(/Private folder/)).not.toBeInTheDocument();
  expect(libraryApi.tasks).toHaveBeenCalledWith(false);
});
