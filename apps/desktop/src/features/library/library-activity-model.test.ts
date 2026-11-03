import { describe, expect, it } from "vitest";
import type { ScanTask } from "../../api/library";
import {
  latestLibraryRefreshes,
  refreshIsRunning,
  refreshNeedsAttention,
} from "./library-activity-model";

const task: ScanTask = {
  id: "refresh",
  root_id: "folder",
  state: "running",
  mode: "automatic",
  phase: "metadata",
  visited_files: 4,
  skipped_files: 0,
  candidate_count: 4,
  imported_count: 4,
  matched_count: 2,
  review_count: 1,
  metadata_failed_count: 0,
  warnings: 0,
  error_code: null,
  created_at: "2026-11-03T10:00:00Z",
  updated_at: "2026-11-03T10:00:01Z",
};

describe("library refresh activity", () => {
  it("replaces an old failed run with the latest successful run for the same folder", () => {
    const current = { ...task, state: "succeeded" as const, review_count: 0 };
    const old = {
      ...task,
      id: "old",
      state: "failed" as const,
      created_at: "2026-11-02T10:00:00Z",
    };
    const latest = latestLibraryRefreshes([old, current]);
    expect(latest).toEqual([current]);
    expect(latest.some(refreshNeedsAttention)).toBe(false);
  });
  it("keeps actual running and exception states separate from navigation or manual review", () => {
    expect(refreshIsRunning(task)).toBe(true);
    expect(refreshNeedsAttention(task)).toBe(false);
    expect(refreshIsRunning({ ...task, state: "waiting_for_user" })).toBe(
      false,
    );
    expect(refreshNeedsAttention({ ...task, state: "succeeded" })).toBe(true);
    expect(
      refreshNeedsAttention({ ...task, state: "interrupted", review_count: 0 }),
    ).toBe(true);
  });
  it("filters by folder and never promotes legacy manual tasks to automatic progress", () => {
    expect(
      latestLibraryRefreshes(
        [
          task,
          { ...task, id: "other", root_id: "other" },
          {
            ...task,
            id: "manual",
            mode: "manual",
            created_at: "2026-11-04T10:00:00Z",
          },
        ],
        "folder",
      ),
    ).toEqual([task]);
  });
});
