import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { expect, it, vi } from "vitest";
import { libraryApi, type ScanTask } from "../../api/library";
import { useLibraryActivity } from "./use-library-activity";

vi.mock("../../api/library", () => ({ libraryApi: { tasks: vi.fn() } }));

it("refreshes visible library data after the final automatic task completion", async () => {
  const running: ScanTask = {
    id: "refresh",
    root_id: "folder",
    mode: "automatic",
    phase: "metadata",
    state: "running",
    visited_files: 1,
    skipped_files: 0,
    candidate_count: 1,
    imported_count: 1,
    matched_count: 0,
    review_count: 0,
    metadata_failed_count: 0,
    warnings: 0,
    error_code: null,
    created_at: "2026-11-03T10:00:00Z",
    updated_at: "2026-11-03T10:00:00Z",
  };
  vi.mocked(libraryApi.tasks).mockResolvedValue([running]);
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  const invalidate = vi.spyOn(client, "invalidateQueries");
  const view = renderHook(() => useLibraryActivity(), {
    wrapper: ({ children }: { children: ReactNode }) => (
      <QueryClientProvider client={client}>{children}</QueryClientProvider>
    ),
  });
  await waitFor(() => expect(view.result.current.active).toHaveLength(1));
  invalidate.mockClear();
  await act(async () => {
    client.setQueryData(
      ["tasks", false],
      [
        {
          ...running,
          state: "succeeded",
          phase: "complete",
          matched_count: 1,
          updated_at: "2026-11-03T10:00:05Z",
        },
      ],
    );
  });
  await waitFor(() =>
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ["media"] }),
  );
  expect(invalidate).toHaveBeenCalledWith({
    queryKey: ["video-source-detail"],
  });
  expect(view.result.current.active).toHaveLength(0);
  expect(view.result.current.attention).toHaveLength(0);
  view.unmount();
  client.clear();
});
