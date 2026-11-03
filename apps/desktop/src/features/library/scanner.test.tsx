import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { I18nextProvider } from "react-i18next";
import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  libraryApi,
  type RootView,
  type ScanCandidate,
  type ScanTask,
} from "../../api/library";
import i18n from "../../i18n";
import { PrivacyProvider } from "./context";
import { ScannerPage } from "./scanner";

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
function show() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  const onSearch = vi.fn();
  render(
    <I18nextProvider i18n={i18n}>
      <QueryClientProvider client={client}>
        <PrivacyProvider>
          <ScannerPage
            search={{ root: root.id, task: task.id }}
            onSearch={onSearch}
          />
        </PrivacyProvider>
      </QueryClientProvider>
    </I18nextProvider>,
  );
  return onSearch;
}

describe("Fixture scan review", () => {
  beforeEach(async () => {
    localStorage.clear();
    await i18n.changeLanguage("en-US");
    vi.mocked(libraryApi.roots).mockResolvedValue([root]);
    vi.mocked(libraryApi.tasks).mockResolvedValue([task]);
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
});
