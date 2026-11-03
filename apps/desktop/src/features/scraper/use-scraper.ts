import {
  useMutation,
  useQueries,
  useQuery,
  useQueryClient,
} from "@tanstack/react-query";
import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { defaultBrowse, libraryApi } from "../../api/library";
import { providerAuthApi } from "../../api/provider-auth";
import { providerApi } from "../../api/providers";
import {
  type MergePreview,
  type OutputPreview,
  type SearchPreview,
  scraperApi,
  scraperOutputApi,
  type VideoKind,
  type VideoMetadata,
} from "../../api/scraper";
import { scrapeKind } from "./scraper-model";

export function useScraper(mediaId?: string) {
  const { i18n } = useTranslation();
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
  async function guarded<T>(action: () => Promise<T>) {
    const epoch = generation.current;
    return { value: await action(), epoch };
  }
  const current = (epoch: number) =>
    active.current && epoch === generation.current;
  const [localSearch, setLocalSearch] = useState("");
  const [localQuery, setLocalQuery] = useState("");
  const [preview, setPreview] = useState<MergePreview>();
  const [output, setOutput] = useState<OutputPreview>();
  const [invalid, setInvalid] = useState(false);
  const [thumbnails, setThumbnails] = useState<Record<number, string>>({});
  useEffect(() => {
    const id = output?.preview_id;
    if (!id) return;
    return () => {
      // Replacing or leaving a preview releases backend leases and never writes.
      void Promise.resolve(scraperOutputApi.discard(id)).catch(() => {
        // Backend leases expire independently if the desktop connection is gone.
      });
    };
  }, [output?.preview_id]);
  const snapshot = useQuery({
    queryKey: ["providers"],
    queryFn: providerApi.snapshot,
  });
  const history = useQuery({
    queryKey: ["scraper-history"],
    queryFn: scraperOutputApi.history,
  });
  const local = useQuery({
    queryKey: ["scraper-local", kind, localQuery],
    enabled: !mediaId,
    queryFn: () =>
      libraryApi.toolsMedia({
        ...defaultBrowse(false),
        kind: kind === "tv" ? "tv_series" : kind,
        query: localQuery || null,
        limit: 48,
      }),
  });
  const targetDetail = useQuery({
    queryKey: ["tools-detail", target],
    queryFn: () => libraryApi.toolsDetail(target),
    enabled: Boolean(target),
  });
  const targetKind = targetDetail.data
    ? scrapeKind(targetDetail.data.media.kind)
    : null;
  const validTarget = Boolean(
    targetDetail.data && targetKind && targetDetail.data.media.id === target,
  );
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
    setPreview(undefined);
    setOutput(undefined);
    setThumbnails({});
    run.reset();
    detail.reset();
    artworkLoad.reset();
    merge.reset();
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
    mutationFn: () =>
      guarded(() =>
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
      ),
    onSuccess: ({ value: result, epoch }) => {
      if (!current(epoch)) return;
      setSearch(result);
      void client.invalidateQueries({ queryKey: ["scraper-history"] });
    },
  });
  const restore = useMutation({
    mutationFn: (runId: string) =>
      guarded(() => scraperApi.searchPreview(runId)),
    onSuccess: ({ value: result, epoch }, runId) => {
      if (!current(epoch)) return;
      clearDetail();
      setSearch(result);
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
    mutationFn: ({ runId, index }: { runId: string; index: number }) =>
      guarded(() => scraperApi.detail(runId, index)),
    onSuccess: ({ value: result, epoch }) => {
      if (!current(epoch)) return;
      setMetadata(result);
      setSelectedFields(result.data.fields.map((f) => f.key));
      setSelectedArtwork([]);
    },
  });
  const artworkLoad = useMutation({
    mutationFn: (index: number) =>
      guarded(() =>
        scraperApi.artworkPreview(
          search?.run_id ?? "",
          candidateIndex ?? 0,
          index,
        ),
      ),
    onSuccess: ({ value: result, epoch }, index) => {
      if (current(epoch))
        setThumbnails((old) => ({ ...old, [index]: result.thumbnail }));
    },
  });
  const merge = useMutation({
    mutationFn: () =>
      guarded(() =>
        scraperApi.preview({
          run_id: search?.run_id ?? "",
          candidate_index: candidateIndex ?? 0,
          media_item_id: target,
          selected_fields: selectedFields,
          selected_artwork: selectedArtwork,
        }),
      ),
    onSuccess: ({ value, epoch }) => {
      if (current(epoch)) setPreview(value);
    },
  });
  const confirm = useMutation({
    mutationFn: () => scraperApi.confirm(preview?.preview_id ?? ""),
    onSuccess: () => {
      void client.invalidateQueries({ queryKey: ["media"] });
      void client.invalidateQueries({ queryKey: ["detail"] });
      void client.invalidateQueries({ queryKey: ["video-source-detail"] });
      void client.invalidateQueries({ queryKey: ["scraper-history"] });
    },
  });
  const outputPlan = useMutation({
    mutationFn: () => guarded(() => scraperOutputApi.preview(target)),
    onSuccess: ({ value, epoch }) => {
      if (current(epoch)) setOutput(value);
      else void scraperOutputApi.discard(value.preview_id).catch(() => {});
    },
  });
  const discard = useMutation({
    mutationFn: () =>
      guarded(() => scraperOutputApi.discard(output?.preview_id ?? "")),
    onSuccess: ({ epoch }) => {
      if (!current(epoch)) return;
      setOutput(undefined);
      write.reset();
    },
  });
  const write = useMutation({
    mutationFn: () => scraperOutputApi.execute(output?.preview_id ?? ""),
    onSuccess: () => {
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
    merge.isPending ||
    confirm.isPending ||
    outputPlan.isPending ||
    write.isPending;
  const error =
    discard.error ??
    restore.error ??
    artworkLoad.error ??
    run.error ??
    detail.error ??
    merge.error ??
    confirm.error ??
    outputPlan.error ??
    write.error;
  const authProviders =
    snapshot.data?.providers.filter(
      (p) =>
        ["tmdb", "anilist", "wikipedia.html"].includes(p.descriptor.id) &&
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
    (title.trim() || (namespace.trim() && externalId.trim())) &&
      (year === "" ||
        (Number.isInteger(Number(year)) &&
          Number(year) >= 1800 &&
          Number(year) <= 2200)) &&
      selectedProviders.length > 0 &&
      (!target || validTarget),
  );
  const selectCandidate = (index: number) => {
    if (!search) return;
    clearDetail();
    setCandidateIndex(index);
    detail.mutate({ runId: search.run_id, index });
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
    setSelectedFields,
    selectedArtwork,
    setSelectedArtwork,
    target,
    targetDetail,
    targetKind,
    validTarget,
    setTarget,
    localSearch,
    setLocalSearch,
    setLocalQuery,
    preview,
    setPreview,
    output,
    setOutput,
    invalid,
    setInvalid,
    thumbnails,
    snapshot,
    history,
    local,
    run,
    restore,
    detail,
    artworkLoad,
    merge,
    confirm,
    outputPlan,
    discard,
    write,
    busy,
    error,
    available,
    supported,
    providerState,
    selectedProviders,
    searchValid,
    selectCandidate,
    clearDetail,
  };
}
