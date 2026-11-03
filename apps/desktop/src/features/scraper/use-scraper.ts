import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { defaultBrowse, libraryApi } from "../../api/library";
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

export function useScraper() {
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
  const [target, setTarget] = useState("");
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
    queryKey: ["scraper-local", metadata?.kind, localQuery],
    enabled: Boolean(metadata),
    queryFn: () =>
      libraryApi.toolsMedia({
        ...defaultBrowse(false),
        kind: metadata?.kind === "tv" ? "tv_series" : (metadata?.kind ?? null),
        query: localQuery || null,
        limit: 48,
      }),
  });
  function clearDetail() {
    setMetadata(undefined);
    setCandidateIndex(undefined);
    setPreview(undefined);
    setOutput(undefined);
    setTarget("");
    setThumbnails({});
    run.reset();
    detail.reset();
    artworkLoad.reset();
    merge.reset();
    outputPlan.reset();
    confirm.reset();
    write.reset();
  }
  const run = useMutation({
    mutationFn: () =>
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
        providers,
      }),
    onSuccess: (result) => {
      setSearch(result);
      void client.invalidateQueries({ queryKey: ["scraper-history"] });
    },
  });
  const restore = useMutation({
    mutationFn: scraperApi.searchPreview,
    onSuccess: (result, runId) => {
      clearDetail();
      setSearch(result);
      const saved = history.data?.find((entry) => entry.id === runId);
      if (saved) {
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
      scraperApi.detail(runId, index),
    onSuccess: (result) => {
      setMetadata(result);
      setSelectedFields(result.data.fields.map((f) => f.key));
      setSelectedArtwork([]);
    },
  });
  const artworkLoad = useMutation({
    mutationFn: (index: number) =>
      scraperApi.artworkPreview(
        search?.run_id ?? "",
        candidateIndex ?? 0,
        index,
      ),
    onSuccess: (result, index) =>
      setThumbnails((old) => ({ ...old, [index]: result.thumbnail })),
  });
  const merge = useMutation({
    mutationFn: () =>
      scraperApi.preview({
        run_id: search?.run_id ?? "",
        candidate_index: candidateIndex ?? 0,
        media_item_id: target,
        selected_fields: selectedFields,
        selected_artwork: selectedArtwork,
      }),
    onSuccess: setPreview,
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
    mutationFn: () => scraperOutputApi.preview(target),
    onSuccess: setOutput,
  });
  const discard = useMutation({
    mutationFn: () => scraperOutputApi.discard(output?.preview_id ?? ""),
    onSuccess: () => {
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
  const available = (id: string) =>
    snapshot.data?.providers.some(
      (p) => p.descriptor.id === id && p.config.enabled,
    );
  const supported = (id: string) =>
    snapshot.data?.providers.some(
      (provider) =>
        provider.descriptor.id === id &&
        provider.descriptor.media_kinds.includes(
          kind === "tv" ? "tv_series" : kind,
        ),
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
    selectCandidate,
    clearDetail,
  };
}
