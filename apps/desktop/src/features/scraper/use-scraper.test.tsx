import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { beforeEach, expect, it, vi } from "vitest";
import { libraryApi, type MediaDetail } from "../../api/library";
import { providerAuthApi } from "../../api/provider-auth";
import { providerApi } from "../../api/providers";
import { scraperApi, scraperOutputApi } from "../../api/scraper";
import i18n from "../../i18n";
import { fixtureSnapshot } from "../providers/fixtures";
import { syntheticMerge, syntheticMetadata, syntheticOutput, syntheticSearch } from "./fixtures";
import { useScraper } from "./use-scraper";

vi.mock("../../components/ui/sonner", () => ({ toast: { success: vi.fn(), info: vi.fn() } }));
vi.mock("../../api/scraper", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../../api/scraper")>();
  return { ...actual, scraperOutputApi: { ...actual.scraperOutputApi, history: vi.fn() } };
});

let local: MediaDetail;
beforeEach(async () => {
  vi.restoreAllMocks();
  await i18n.changeLanguage("en-US");
  local = {
    media: { id: "media-1", kind: "movie", title: "Local film", original_title: null, year: 2024, created_at: "2026-01-01", updated_at: "2026-01-01" },
    files: [], artworks: [], external_ids: [], metadata_values: [], health: ["missing_nfo"], relations: [], poster_uri: null,
  };
  const snapshot = fixtureSnapshot();
  snapshot.providers[0].descriptor.id = "tmdb";
  snapshot.providers[0].descriptor.media_kinds = ["movie"];
  snapshot.providers[0].config.enabled = true;
  vi.spyOn(providerApi, "snapshot").mockResolvedValue(snapshot);
  vi.spyOn(providerAuthApi, "status").mockResolvedValue({
    provider_id: "tmdb", supported_methods: ["bearer"], method: "bearer", storage: "system", configured: true, status: "configured",
  });
  vi.spyOn(libraryApi, "toolsDetail").mockImplementation(async () => local);
  vi.mocked(scraperOutputApi.history).mockResolvedValue([]);
  vi.spyOn(scraperOutputApi, "defaultMode").mockResolvedValue("online");
  vi.spyOn(scraperOutputApi, "preview").mockResolvedValue(syntheticOutput);
  vi.spyOn(scraperOutputApi, "discard").mockResolvedValue(null);
  vi.spyOn(scraperOutputApi, "execute").mockImplementation(async () => {
    local = { ...local, health: [] };
    return { schema_version: 1, preview_id: "output-1", written: 1, preserved: 0, already_applied: false, state: "completed" };
  });
  vi.spyOn(scraperApi, "search").mockResolvedValue(syntheticSearch);
  vi.spyOn(scraperApi, "detail").mockResolvedValue(syntheticMetadata);
  vi.spyOn(scraperApi, "preview").mockImplementation(async (request) => ({
    ...syntheticMerge,
    fields: syntheticMerge.fields.map((field) => ({ ...field, selected: request.selected_fields.includes(field.key) && !field.protected })),
  }));
  vi.spyOn(scraperApi, "confirm").mockImplementation(async () => {
    local = {
      ...local,
      media: { ...local.media, title: "Saved local title" },
      metadata_values: [{ id: "saved-overview", field_key: "overview", locale: "en-US", value: "Synthetic provider summary", provider_id: "tmdb", source_url: null, is_locked: false, is_user_modified: false }],
    };
    return { schema_version: 1, preview_id: "merge-1", applied_fields: 1, already_applied: false };
  });
});

async function workspace() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  for (const key of ["tools-media", "scraper-local", "facets"])
    client.setQueryData([key, "cached-scope"], { items: [] });
  client.setQueryData(["adult", "tools-media", "private-scope"], { items: [] });
  const hook = renderHook(() => useScraper("media-1"), {
    wrapper: ({ children }: { children: ReactNode }) => <QueryClientProvider client={client}>{children}</QueryClientProvider>,
  });
  await waitFor(() => expect(hook.result.current.searchValid).toBe(true));
  return { ...hook, client };
}

it("refreshes the actual local merge baseline and marks all ordinary list consumers stale after confirmation", async () => {
  const { result, client } = await workspace();
  act(() => result.current.run.mutate());
  await waitFor(() => expect(result.current.search).toBeDefined());
  act(() => result.current.selectCandidate(0));
  await waitFor(() => expect(result.current.mergeSelectionValid).toBe(true));
  act(() => result.current.confirm.mutate());
  await waitFor(() => expect(result.current.confirm.isSuccess).toBe(true));
  expect(result.current.targetDetail.data?.media.title).toBe("Saved local title");
  expect(result.current.targetDetail.data?.metadata_values[0].value).toBe("Synthetic provider summary");
  act(() => result.current.applyFieldPreset("missing"));
  expect(result.current.selectedFields).not.toContain("overview");
  for (const key of ["tools-media", "scraper-local", "facets"])
    expect(client.getQueryState([key, "cached-scope"])?.isInvalidated).toBe(true);
  expect(client.getQueryState(["adult", "tools-media", "private-scope"])?.isInvalidated).toBe(false);
  expect(scraperOutputApi.execute).not.toHaveBeenCalled();
});

it("refreshes NFO health after explicit output and permits another preview after closing a completed one", async () => {
  const { result } = await workspace();
  await waitFor(() => expect(result.current.outputModeReady).toBe(true));
  act(() => result.current.outputPlan.mutate());
  await waitFor(() => expect(result.current.output).toBeDefined());
  expect(scraperOutputApi.execute).not.toHaveBeenCalled();
  act(() => result.current.write.mutate());
  await waitFor(() => expect(result.current.write.isSuccess).toBe(true));
  expect(result.current.targetDetail.data?.health).toEqual([]);
  act(() => result.current.discard.mutate());
  await waitFor(() => expect(result.current.output).toBeUndefined());
  expect(result.current.write.data?.written).toBe(1);
  vi.mocked(scraperOutputApi.preview).mockResolvedValue({ ...syntheticOutput, preview_id: "output-2" });
  act(() => result.current.outputPlan.mutate());
  await waitFor(() => expect(result.current.output?.preview_id).toBe("output-2"));
  expect(result.current.write.isSuccess).toBe(false);
  expect(scraperOutputApi.execute).toHaveBeenCalledTimes(1);
  expect(scraperApi.confirm).not.toHaveBeenCalled();
});

it.each(["kind", "language"])("keeps incompatible restored %s results read-only until another search", async (field) => {
  vi.mocked(scraperOutputApi.history).mockResolvedValue([{
    id: "saved-run", state: "succeeded", created_at: "2026-01-01", candidate_count: 1,
    query: { kind: field === "kind" ? "tv" : "movie", title: "Saved query", year: 2024, external_ids: [], language: field === "language" ? "zh-CN" : "en-US" },
  }]);
  vi.spyOn(scraperApi, "searchPreview").mockResolvedValue(syntheticSearch);
  const { result } = await workspace();
  await waitFor(() => expect(result.current.history.data).toHaveLength(1));
  act(() => result.current.restore.mutate("saved-run"));
  await waitFor(() => expect(result.current.restore.isSuccess).toBe(true));
  expect(result.current.kind).toBe("movie");
  expect(result.current.title).toBe("Saved query");
  expect(result.current.staleSearch).toBe(true);
  act(() => result.current.selectCandidate(0));
  expect(scraperApi.detail).not.toHaveBeenCalled();
  expect(scraperApi.search).not.toHaveBeenCalled();
});
