import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import type { MediaDetail } from "../../api/library";
import { DesktopError } from "../../api/desktop";
import i18n from "../../i18n";
import { syntheticMerge, syntheticMetadata } from "./fixtures";
import { buildMergePreview } from "./merge-preview-model";
import { MetadataMergeWorkspace } from "./merge-workspace";
import type { useScraper } from "./use-scraper";
import "./translations";
import "./review-queue-translations";

vi.mock("./source-detail", () => ({ VideoSourceDetails: () => null }));

type Workspace = ReturnType<typeof useScraper>;
const local: MediaDetail = {
  media: {
    id: "media-1", kind: "movie", title: "Saved local title", original_title: null,
    year: 2024, created_at: "2026-11-06", updated_at: "2026-11-06",
  },
  files: [], artworks: [], metadata_values: [], relations: [], health: [], poster_uri: null,
  external_ids: [{ id: "id-1", provider_id: "tmdb", namespace: "tmdb", external_id: "fixture-1" }],
};

function workspace(): Workspace {
  return {
    metadata: syntheticMetadata,
    projection: buildMergePreview({ metadata: syntheticMetadata, target: local, selectedFields: [], selectedArtwork: [], onlyMissing: false }),
    targetDetail: { data: local },
    validTarget: true,
    targetKind: "movie",
    identityChanges: [],
    thumbnails: {}, selectedFields: [], selectedArtwork: [], onlyMissing: false,
    applyFieldPreset: vi.fn(), chooseFields: vi.fn(), chooseArtwork: vi.fn(), setOnlyMissing: vi.fn(),
    artworkLoad: { mutate: vi.fn() },
    confirm: { isSuccess: false, isPending: false, mutate: vi.fn() },
    write: { isPending: false, mutate: vi.fn() },
    discard: { isPending: false },
    outputPlan: { mutate: vi.fn() },
    outputMode: "online", outputModeReady: true, outputModeLoading: false,
    setOutputMode: vi.fn(), retryOutputMode: vi.fn(),
    mergePreviewPending: false, mergeSelectionValid: false,
    retryMergePreview: vi.fn(),
  } as unknown as Workspace;
}

beforeEach(async () => { await i18n.changeLanguage("en-US"); });

function show(value: Workspace, busy = false) {
  return render(<MetadataMergeWorkspace workspace={value} busy={busy} onCancel={vi.fn()} />);
}

it("previews saved metadata with zero changes without confirming the remote fields or writing files", () => {
  const value = workspace();
  show(value);
  expect(screen.getByText("Output uses saved metadata. Unconfirmed source fields are excluded.")).toBeVisible();
  expect(screen.getByRole("button", { name: "Confirm merge (0 changes)" })).toBeDisabled();
  expect(screen.queryByText("Confirmed")).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Preview saved metadata output" }));
  expect(value.outputPlan.mutate).toHaveBeenCalledExactlyOnceWith();
  expect(value.confirm.mutate).not.toHaveBeenCalled();
  expect(value.write.mutate).not.toHaveBeenCalled();
});

it("removes the saved-output shortcut when the user selects a field to change", () => {
  const value = workspace();
  const view = show(value);
  expect(screen.getByRole("button", { name: "Preview saved metadata output" })).toBeEnabled();
  const changed = {
    ...value,
    selectedFields: ["overview"],
    projection: buildMergePreview({ metadata: syntheticMetadata, target: local, selectedFields: ["overview"], selectedArtwork: [], onlyMissing: false }),
  };
  view.rerender(<MetadataMergeWorkspace workspace={changed} busy={false} onCancel={vi.fn()} />);
  expect(screen.queryByRole("button", { name: "Preview saved metadata output" })).not.toBeInTheDocument();
  expect(screen.getByRole("button", { name: /^Confirm merge/ })).toBeDisabled();
  expect(value.outputPlan.mutate).not.toHaveBeenCalled();
});

it.each(["identity", "pending", "error", "mismatch", "anonymous", "type"] as const)(
  "does not expose the zero-change output shortcut for %s", (condition) => {
    const value = workspace();
    if (condition === "identity") value.identityChanges = [{ namespace: "tmdb", old: ["old"], new: ["new"] }];
    if (condition === "pending") value.mergePreviewPending = true;
    if (condition === "error") value.mergePreviewError = new DesktopError("request_failed");
    if (condition === "mismatch") value.preview = syntheticMerge;
    if (condition === "anonymous") value.validTarget = false;
    if (condition === "type") value.targetKind = "tv";
    show(value);
    expect(screen.queryByRole("button", { name: "Preview saved metadata output" })).not.toBeInTheDocument();
    expect(value.outputPlan.mutate).not.toHaveBeenCalled();
    expect(value.confirm.mutate).not.toHaveBeenCalled();
    expect(value.write.mutate).not.toHaveBeenCalled();
  },
);

it.each(["busy", "mode"] as const)("keeps output planning disabled while %s is unresolved", (condition) => {
  const value = workspace();
  if (condition === "mode") value.outputModeReady = false;
  show(value, condition === "busy");
  const preview = screen.getByRole("button", { name: "Preview saved metadata output" });
  expect(preview).toBeDisabled();
  fireEvent.click(preview);
  expect(value.outputPlan.mutate).not.toHaveBeenCalled();
});

it("keeps output-mode retry available before a zero-change export", () => {
  const value = workspace();
  value.outputModeReady = false;
  value.outputModeError = new DesktopError("request_failed");
  show(value);
  fireEvent.click(screen.getByRole("button", { name: "Retry" }));
  expect(value.retryOutputMode).toHaveBeenCalledOnce();
  expect(value.outputPlan.mutate).not.toHaveBeenCalled();
});

it("retains the confirmed-metadata export flow", () => {
  const value = workspace();
  value.confirm.isSuccess = true;
  show(value);
  expect(screen.getByText("Confirmed")).toBeVisible();
  expect(screen.queryByRole("button", { name: /^Confirm merge/ })).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: i18n.t("scraperOutputPreview") }));
  expect(value.outputPlan.mutate).toHaveBeenCalledOnce();
  expect(value.write.mutate).not.toHaveBeenCalled();
});
