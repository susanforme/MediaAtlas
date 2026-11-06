import { fireEvent, render, screen } from "@testing-library/react";
import type { ReactNode } from "react";
import { I18nextProvider } from "react-i18next";
import { beforeEach, expect, it, vi } from "vitest";
import type { RootView, ScanTask } from "../../api/library";
import i18n from "../../i18n";
import { RootCard } from "./root-card";
import "./translations";
import "./library-status-translations";

vi.mock("@tanstack/react-router", () => ({
  Link: ({ to, search, children }: { to: string; search?: unknown; children: ReactNode }) => (
    <a href={to} data-search={JSON.stringify(search)}>{children}</a>
  ),
}));

const time = "2026-11-03T00:00:00Z";
const root: RootView = {
  id: "movies", name: "Movies", path: "/fixture", path_style: "posix", kind: "movie",
  enabled: true, exclude_patterns: [], last_scan_at: null, created_at: time, updated_at: time,
};
const task: ScanTask = {
  id: "update", root_id: root.id, state: "running", visited_files: 8, skipped_files: 0,
  candidate_count: 8, imported_count: 8, warnings: 0, error_code: null,
  created_at: time, updated_at: time, mode: "automatic", phase: "metadata",
};

beforeEach(async () => { await i18n.changeLanguage("en-US"); });

function show(currentTask = task, currentRoot = root) {
  const refresh = vi.fn();
  const remove = vi.fn();
  render(
    <I18nextProvider i18n={i18n}>
      <RootCard root={currentRoot} task={currentTask} refreshing={false} disabled={false}
        onEdit={vi.fn()} onRemove={remove} onRefresh={refresh} />
    </I18nextProvider>,
  );
  return { refresh, remove };
}

it("opens the current background task while keeping duplicate update and removal unavailable", () => {
  const { refresh, remove } = show();
  expect(screen.getByRole("link", { name: "Details" })).toHaveAttribute(
    "data-search", JSON.stringify({ root: root.id, task: task.id }),
  );
  const update = screen.getByRole("button", { name: "Updating…" });
  const deletion = screen.getByRole("button", { name: "Remove" });
  expect(update).toBeDisabled();
  expect(deletion).toBeDisabled();
  fireEvent.click(update);
  fireEvent.click(deletion);
  expect(refresh).not.toHaveBeenCalled();
  expect(remove).not.toHaveBeenCalled();
});

it("links completed updates with unresolved metadata to their results", () => {
  show({ ...task, state: "succeeded", phase: "complete", metadata_failed_count: 1 });
  expect(screen.getByRole("link", { name: "View results" })).toHaveAttribute(
    "data-search", JSON.stringify({ root: root.id, task: task.id }),
  );
});

it("keeps the private scope on a running task link", () => {
  show(task, { ...root, kind: "adult_video" });
  expect(screen.getByRole("link", { name: "Details" })).toHaveAttribute(
    "data-search", JSON.stringify({ root: root.id, task: task.id, kind: "adult_video" }),
  );
});
