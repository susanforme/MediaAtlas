import {
  useMutation,
  useQueries,
  useQuery,
  useQueryClient,
} from "@tanstack/react-query";
import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
} from "react";
import { useTranslation } from "react-i18next";
import { DesktopError } from "../../api/desktop";
import { providerAuthApi } from "../../api/provider-auth";
import {
  type NfoExportMode,
  type OutputPreview,
  type SearchPreview,
  scraperApi,
  scraperOutputApi,
  type VideoKind,
  type VideoMetadata,
} from "../../api/scraper";
import { toast } from "../../components/ui/sonner";
import { providersQuery } from "../providers/queries";
import { toolsDetailQuery } from "../tools/queries";
import { outputDefaultQuery, scraperHistoryQuery, scraperLocalQuery } from "./queries";
import { type FieldPreset, selectScraperFields } from "./field-selection";
import {
  buildMergePreview,
  mergeSelectionMatches,
} from "./merge-preview-model";
import { changedExternalIds, identityMayChange, providerChoices, scrapeKind } from "./scraper-model";
import { useLiveMergePreview } from "./use-live-merge-preview";

type PreviewAction = { previewId: string; epoch: number };
type OutputAction = PreviewAction & { outputEpoch: number };
type OutputPlanAction = {
  mediaId: string;
  mode: NfoExportMode;
  override: boolean;
  epoch: number;
  outputEpoch: number;
};

export function useScraper(
  mediaId?: string,
  review?: {
    confirmPreview: (
      previewId: string,
    ) => ReturnType<typeof scraperApi.confirm>;
    failed: () => void;
  },
) {
  const { t, i18n } = useTranslation();
  const client = useQueryClient();
  const [kind, setKind] = useState<VideoKind>("movie");
  const [title, setTitle] = useState("");
  const [year, setYear] = useState("");
  const [namespace, setNamespace] = useState("");
  const [externalId, setExternalId] = useState("");
  const [providers, setProviders] = useState<string[]>(["tmdb"]);
  const [search, setSearch] = useState<SearchPreview>();
  const [metadata, setMetadata] = useState<VideoMetadata>();
  const [candidateIndex, setCandidateIndex] = useState<number>();
  const [selectedFields, setSelectedFields] = useState<string[]>([]);
  const [selectedArtwork, setSelectedArtwork] = useState<number[]>([]);
  const [selectedTarget, setSelectedTarget] = useState("");
  const target = mediaId ?? selectedTarget;
  const generation = useRef(0);
  const active = useRef(true);
  useEffect(() => {
    active.current = true;
    return () => {
      active.current = false;
      generation.current += 1;
    };
  }, []);
  async function guarded<T>(action: () => Promise<T>, epoch: number) {
    return { value: await action(), epoch };
  }
  const current = (epoch: number) =>
    active.current && epoch === generation.current;
  const [localSearch, setLocalSearch] = useState("");
  const [localQuery, setLocalQuery] = useState("");
  const [onlyMissing, updateOnlyMissing] = useState(false);
  const [output, setOutput] = useState<OutputPreview>();
  const [outputChoice, updateOutputChoice] = useState<{ target: string; mode: NfoExportMode }>();
  const outputGeneration = useRef(0);
  const [invalid, setInvalid] = useState(false);
  const [thumbnails, setThumbnails] = useState<Record<number, string>>({});
  const outputReleases = useRef(new Map<string, Promise<null>>());
  const releaseOutput = useCallback((previewId: string) => {
    const existing = outputReleases.current.get(previewId);
    if (existing) return existing;
    const request = Promise.resolve().then(() =>
      scraperOutputApi.discard(previewId),
    );
    outputReleases.current.set(previewId, request);
    void request.catch(() => outputReleases.current.delete(previewId));
    return request;
  }, []);
  useEffect(() => {
    const id = output?.preview_id;
    if (!id) return;
    return () => {
      // Replacing or leaving a preview releases backend leases and never writes.
      void releaseOutput(id).catch(() => {
        // Backend leases expire independently if the desktop connection is gone.
      });
    };
  }, [output?.preview_id, releaseOutput]);
  const snapshot = useQuery(providersQuery());
  const history = useQuery(scraperHistoryQuery);
  const local = useQuery({
    ...scraperLocalQuery(kind, localQuery),
    enabled: !mediaId,
  });
  const targetDetail = useQuery({
    ...toolsDetailQuery(target),
    enabled: Boolean(target),
  });
  const targetKind = targetDetail.data
    ? scrapeKind(targetDetail.data.media.kind)
    : null;
  const validTarget = Boolean(
    targetDetail.data && targetKind && targetDetail.data.media.id === target,
  );
  const outputDefault = useQuery({
    ...outputDefaultQuery(target),
    enabled: validTarget,
  });
  const outputOverride = outputChoice?.target === target ? outputChoice.mode : undefined;
  const outputMode = outputOverride ?? outputDefault.data ?? "online";
  const outputModeReady = validTarget && Boolean(
    outputOverride || (outputDefault.isSuccess && !outputDefault.isFetching && !outputDefault.isPlaceholderData),
  );
  const outputContext = useRef({ target, mode: outputMode, ready: outputModeReady });
  const draftProjection = metadata
    ? buildMergePreview({
        metadata,
        target: validTarget ? targetDetail.data : undefined,
        selectedFields,
        selectedArtwork,
        onlyMissing,
      })
    : undefined;
  const liveMerge = useLiveMergePreview(
    validTarget &&
      metadata && metadata.kind === targetKind &&
      search &&
      candidateIndex !== undefined &&
      draftProjection &&
      (draftProjection.counts.total > 0 || identityMayChange(metadata, targetDetail.data))
      ? {
          run_id: search.run_id,
          candidate_index: candidateIndex,
          media_item_id: target,
          selected_fields: draftProjection.effectiveFields,
          selected_artwork: draftProjection.effectiveArtwork,
        }
      : null,
  );
  const preview = liveMerge.preview;
  const projection = metadata
    ? buildMergePreview({
        metadata,
        target: validTarget ? targetDetail.data : undefined,
        selectedFields,
        selectedArtwork,
        onlyMissing,
        preview,
      })
    : undefined;
  const mergeSelectionValid = Boolean(
    preview && projection && mergeSelectionMatches(preview, projection),
  );
  const identityChanges = changedExternalIds(preview);
  const mergeChangeCount = (projection?.counts.total ?? 0) + identityChanges.length;
  const latestActions = useRef({
    epoch: generation.current,
    mediaId: target,
    outputMode,
    outputModeReady,
    mergeId: preview?.preview_id,
    mergeReady: false,
    outputId: output?.preview_id,
  });
  // A DOM action can arrive before useMutation's passive option update.
  // Bind it to the committed preview rather than a mutationFn render closure.
  useLayoutEffect(() => {
    const previous = outputContext.current;
    const outputChanged = previous.target !== target || previous.mode !== outputMode || previous.ready !== outputModeReady;
    if (outputChanged) {
      outputContext.current = { target, mode: outputMode, ready: outputModeReady };
      outputGeneration.current += 1;
      if (previous.target !== target) updateOutputChoice(undefined);
      setOutput(undefined);
      outputPlan.reset();
      if (!discard.isPending) discard.reset();
      if (!write.isPending) write.reset();
    }
    latestActions.current = {
      epoch: generation.current,
      mediaId: target,
      outputMode,
      outputModeReady,
      mergeId: preview?.preview_id,
      mergeReady: Boolean(
        mergeSelectionValid && !liveMerge.pending && mergeChangeCount,
      ),
      outputId: outputChanged ? undefined : output?.preview_id,
    };
  });
  function validAction(action: PreviewAction, kind: "merge" | "output") {
    const latest = latestActions.current;
    return (
      current(action.epoch) &&
      latest.epoch === action.epoch &&
      Boolean(action.previewId) &&
      (kind === "merge"
        ? latest.mergeReady && latest.mergeId === action.previewId
        : latest.outputModeReady && latest.outputId === action.previewId)
    );
  }
  const seededTarget = useRef("");
  useEffect(() => {
    if (
      !targetDetail.data ||
      !targetKind ||
      targetDetail.data.media.id !== target ||
      seededTarget.current === target
    )
      return;
    seededTarget.current = target;
    setTitle(targetDetail.data.media.title);
    setYear(
      targetDetail.data.media.year === null
        ? ""
        : String(targetDetail.data.media.year),
    );
    setKind(targetKind);
    setProviders(targetKind === "anime" ? ["anilist"] : ["tmdb"]);
  }, [targetDetail.data, targetKind, target]);
  function clearDetail() {
    generation.current += 1;
    setMetadata(undefined);
    setCandidateIndex(undefined);
    setOutput(undefined);
    setThumbnails({});
    run.reset();
    detail.reset();
    artworkLoad.reset();
    outputPlan.reset();
    discard.reset();
    confirm.reset();
    write.reset();
  }
  function setTarget(value: string) {
    clearDetail();
    restore.reset();
    setSearch(undefined);
    seededTarget.current = "";
    setSelectedTarget(value);
    if (!value) {
      setTitle("");
      setYear("");
    }
  }
  const run = useMutation({
    mutationFn: (epoch: number) =>
      guarded(
        () =>
          scraperApi.search({
            query: {
              kind,
              title: title.trim(),
              year: year === "" ? null : Number(year),
              external_ids: externalId.trim()
                ? [{ namespace: namespace.trim(), value: externalId.trim() }]
                : [],
              language: i18n.resolvedLanguage === "en-US" ? "en-US" : "zh-CN",
            },
            providers: selectedProviders,
          }),
        epoch,
      ),
    onSuccess: ({ value: result, epoch }) => {
      if (!current(epoch)) return;
      setSearch(result);
      if (!result.candidates.length && result.failures.length) review?.failed();
      void client.invalidateQueries({ queryKey: ["scraper-history"] });
    },
    onError: (_error, epoch) => {
      if (current(epoch)) review?.failed();
    },
  });
  const restore = useMutation({
    mutationFn: ({ runId, epoch }: { runId: string; epoch: number }) =>
      guarded(() => scraperApi.searchPreview(runId), epoch),
    onSuccess: ({ value: result, epoch }, { runId }) => {
      if (!current(epoch)) return;
      clearDetail();
      setSearch(result);
      toast.info(t("scraperRestored"));
      const saved = history.data?.find((entry) => entry.id === runId);
      if (saved && !target) {
        setKind(saved.query.kind);
        setTitle(saved.query.title);
        setYear(saved.query.year === null ? "" : String(saved.query.year));
        setNamespace(saved.query.external_ids[0]?.namespace ?? "");
        setExternalId(saved.query.external_ids[0]?.value ?? "");
        setProviders([
          ...new Set(
            result.candidates.map(
              (candidate) => candidate.candidate.data.provider_id,
            ),
          ),
        ]);
      }
    },
  });
  const detail = useMutation({
    mutationFn: ({
      runId,
      index,
      epoch,
    }: {
      runId: string;
      index: number;
      epoch: number;
    }) => guarded(() => scraperApi.detail(runId, index), epoch),
    onSuccess: ({ value: result, epoch }) => {
      if (!current(epoch)) return;
      setMetadata(result);
      setSelectedFields(result.data.fields.map((f) => f.key));
      setSelectedArtwork([]);
    },
    onError: (_error, { epoch }) => {
      if (current(epoch)) review?.failed();
    },
  });
  const artworkLoad = useMutation({
    mutationFn: ({ index, epoch }: { index: number; epoch: number }) =>
      guarded(
        () =>
          scraperApi.artworkPreview(
            search?.run_id ?? "",
            candidateIndex ?? 0,
            index,
          ),
        epoch,
      ),
    onSuccess: ({ value: result, epoch }, { index }) => {
      if (current(epoch))
        setThumbnails((old) => ({ ...old, [index]: result.thumbnail }));
    },
  });
  const confirm = useMutation({
    mutationFn: (action: PreviewAction) => {
      if (!validAction(action, "merge"))
        throw new DesktopError("request_failed");
      return review
        ? review.confirmPreview(action.previewId)
        : scraperApi.confirm(action.previewId);
    },
    onSuccess: (result, action) => {
      if (!current(action.epoch)) return;
      toast.success(
        t(result.already_applied ? "scraperAlready" : "scraperConfirmed", {
          count: result.applied_fields,
        }),
      );
      void client.invalidateQueries({ queryKey: ["media"] });
      void client.invalidateQueries({ queryKey: ["detail"] });
      void client.invalidateQueries({ queryKey: ["video-source-detail"] });
      void client.invalidateQueries({ queryKey: ["scraper-history"] });
    },
  });
  const outputPlan = useMutation({
    mutationFn: (action: OutputPlanAction) => {
      const latest = latestActions.current;
      if (
        !current(action.epoch) ||
        action.outputEpoch !== outputGeneration.current ||
        action.mediaId !== latest.mediaId ||
        !latest.outputModeReady ||
        action.mode !== latest.outputMode
      )
        throw new DesktopError("request_failed");
      return guarded(
        () => action.override
          ? scraperOutputApi.preview(action.mediaId, action.mode)
          : scraperOutputApi.preview(action.mediaId, undefined, action.mode),
        action.epoch,
      );
    },
    onSuccess: ({ value, epoch }, action) => {
      if (current(epoch) && action.outputEpoch === outputGeneration.current &&
        action.mediaId === latestActions.current.mediaId &&
        action.mode === latestActions.current.outputMode && latestActions.current.outputModeReady)
        setOutput(value);
      else void releaseOutput(value.preview_id).catch(() => {});
    },
    onError: (error, action) => {
      if (error instanceof DesktopError && error.envelope?.code === "conflict" &&
        !action.override && current(action.epoch) && action.outputEpoch === outputGeneration.current)
        void client.invalidateQueries({ queryKey: ["roots", "nfo-export-mode", action.mediaId] });
    },
  });
  const discard = useMutation({
    mutationFn: (action: OutputAction) => {
      if (
        action.outputEpoch !== outputGeneration.current ||
        !validAction(action, "output")
      )
        throw new DesktopError("request_failed");
      return guarded(() => releaseOutput(action.previewId), action.epoch);
    },
    onSuccess: ({ epoch }, action) => {
      if (
        !current(epoch) ||
        latestActions.current.outputId !== action.previewId
      )
        return;
      setOutput(undefined);
      write.reset();
    },
  });
  const write = useMutation({
    mutationFn: (action: OutputAction) => {
      if (
        action.outputEpoch !== outputGeneration.current ||
        !validAction(action, "output")
      )
        throw new DesktopError("request_failed");
      return scraperOutputApi.execute(action.previewId);
    },
    onSuccess: (result, action) => {
      if (!current(action.epoch)) return;
      toast.success(
        t("scraperOutputDone", {
          written: result.written,
          preserved: result.preserved,
        }),
      );
      void client.invalidateQueries({ queryKey: ["media"] });
      void client.invalidateQueries({ queryKey: ["detail"] });
      void client.invalidateQueries({ queryKey: ["video-source-detail"] });
    },
  });
  const busy =
    discard.isPending ||
    artworkLoad.isPending ||
    restore.isPending ||
    run.isPending ||
    detail.isPending ||
    confirm.isPending ||
    outputPlan.isPending ||
    write.isPending;
  const error =
    discard.error ??
    restore.error ??
    artworkLoad.error ??
    run.error ??
    detail.error ??
    confirm.error ??
    outputPlan.error ??
    write.error;
  const authProviders =
    snapshot.data?.providers.filter(
      (p) =>
        providerChoices.includes(p.descriptor.id) &&
        (p.descriptor.requires_api_key || p.descriptor.requires_login),
    ) ?? [];
  const auth = useQueries({
    queries: authProviders.map((p) => ({
      queryKey: ["provider-auth", p.descriptor.id],
      queryFn: () => providerAuthApi.status(p.descriptor.id),
      retry: false,
    })),
  });
  const providerState = (id: string) => {
    const provider = snapshot.data?.providers.find(
      (p) => p.descriptor.id === id,
    );
    if (!provider) return "missing";
    if (!provider.config.enabled) return "disabled";
    if (
      !provider.descriptor.media_kinds.includes(
        kind === "tv" ? "tv_series" : kind,
      )
    )
      return "unsupported";
    const index = authProviders.findIndex((p) => p.descriptor.id === id);
    if (index !== -1) {
      if (auth[index].isPending) return "checking";
      if (
        auth[index].isError ||
        !(
          auth[index].data?.configured ||
          auth[index].data?.status === "not_required"
        )
      )
        return "credentials";
    }
    return "ready";
  };
  const available = (id: string) => providerState(id) === "ready";
  const supported = (id: string) =>
    snapshot.data?.providers.some(
      (p) =>
        p.descriptor.id === id &&
        p.descriptor.media_kinds.includes(kind === "tv" ? "tv_series" : kind),
    );
  const selectedProviders = providers.filter(available);
  const searchValid = Boolean(
    (title.trim() || externalId.trim()) &&
      (year === "" ||
        (Number.isInteger(Number(year)) &&
          Number(year) >= 1800 &&
          Number(year) <= 2200)) &&
      selectedProviders.length > 0 &&
      (!target || validTarget),
  );
  function applyFieldPreset(preset: FieldPreset) {
    if (!metadata || busy || (preset === "missing" && !validTarget)) return;
    const fields = selectScraperFields(
      preset,
      metadata.data.fields,
      validTarget ? targetDetail.data : undefined,
      preview,
    );
    generation.current += 1;
    setSelectedFields(fields);
    setOutput(undefined);
    outputPlan.reset();
    discard.reset();
    confirm.reset();
    write.reset();
  }
  function resetMergeResult() {
    setOutput(undefined);
    outputPlan.reset();
    confirm.reset();
    write.reset();
  }
  function setOutputMode(mode: NfoExportMode) {
    if (
      (mode === outputMode && outputOverride !== undefined) || write.isPending || discard.isPending || confirm.isPending
    ) return;
    outputGeneration.current += 1;
    latestActions.current.outputId = undefined;
    latestActions.current.outputMode = mode;
    latestActions.current.outputModeReady = true;
    updateOutputChoice({ target, mode });
    setOutput(undefined);
    outputPlan.reset();
    discard.reset();
    write.reset();
  }
  function chooseFields(fields: string[]) {
    if (busy) return;
    resetMergeResult();
    setSelectedFields(fields);
  }
  function chooseArtwork(artwork: number[]) {
    if (busy) return;
    resetMergeResult();
    setSelectedArtwork(artwork);
  }
  function setOnlyMissing(value: boolean) {
    if (busy) return;
    resetMergeResult();
    updateOnlyMissing(value);
  }
  const selectCandidate = (index: number) => {
    if (!search) return;
    clearDetail();
    setCandidateIndex(index);
    detail.mutate({ runId: search.run_id, index, epoch: generation.current });
  };
  return {
    kind,
    setKind,
    title,
    setTitle,
    year,
    setYear,
    namespace,
    setNamespace,
    externalId,
    setExternalId,
    providers,
    setProviders,
    search,
    setSearch,
    metadata,
    candidateIndex,
    selectedFields,
    chooseFields,
    selectedArtwork,
    chooseArtwork,
    target,
    targetDetail,
    targetKind,
    validTarget,
    setTarget,
    localSearch,
    setLocalSearch,
    setLocalQuery,
    preview,
    projection,
    onlyMissing,
    setOnlyMissing,
    mergePreviewPending: liveMerge.pending,
    mergePreviewError: liveMerge.error,
    mergeSelectionValid,
    identityChanges,
    retryMergePreview: () => {
      const epoch = generation.current;
      void targetDetail.refetch().then(() => {
        if (current(epoch)) liveMerge.retry();
      });
    },
    output,
    setOutput,
    outputMode,
    outputModeReady,
    outputModeLoading: !outputOverride && outputDefault.isFetching,
    outputModeError: outputOverride ? undefined : outputDefault.error,
    retryOutputMode: () => void outputDefault.refetch(),
    setOutputMode,
    invalid,
    setInvalid,
    thumbnails,
    snapshot,
    history,
    local,
    run: { ...run, mutate: () => run.mutate(generation.current) },
    restore: {
      ...restore,
      mutate: (runId: string) =>
        restore.mutate({ runId, epoch: generation.current }),
    },
    detail,
    artworkLoad: {
      ...artworkLoad,
      mutate: (index: number) =>
        artworkLoad.mutate({ index, epoch: generation.current }),
    },
    confirm: {
      ...confirm,
      mutate: () =>
        confirm.mutate({
          previewId: preview?.preview_id ?? "",
          epoch: generation.current,
        }),
    },
    outputPlan: {
      ...outputPlan,
      mutate: () =>
        outputPlan.mutate({
          mediaId: target,
          mode: outputMode,
          override: outputOverride !== undefined,
          epoch: generation.current,
          outputEpoch: outputGeneration.current,
        }),
    },
    discard: {
      ...discard,
      mutate: () =>
        discard.mutate({
          previewId: output?.preview_id ?? "",
          epoch: generation.current,
          outputEpoch: outputGeneration.current,
        }),
    },
    write: {
      ...write,
      mutate: () =>
        write.mutate({
          previewId: output?.preview_id ?? "",
          epoch: generation.current,
          outputEpoch: outputGeneration.current,
        }),
    },
    busy,
    error,
    available,
    supported,
    providerState,
    selectedProviders,
    searchValid,
    applyFieldPreset,
    selectCandidate,
    clearDetail,
  };
}
