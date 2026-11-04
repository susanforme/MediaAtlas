import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import {
  type MusicAlbumView,
  type MusicCoverPreview,
  type MusicReleaseGraph,
  musicApi,
} from "../../api/music";
import { Badge } from "../../components/ui/badge";
import { Anchor, Button } from "../../components/ui/button";
import { Card } from "../../components/ui/card";
import { Checkbox } from "../../components/ui/checkbox";
import { Details, DetailsSummary } from "../../components/ui/details";
import { Input } from "../../components/ui/input";
import { Label } from "../../components/ui/label";
import { Select } from "../../components/ui/select";
import { toast } from "../../components/ui/sonner";
import {
  Table,
  TableHeader,
  TableRow,
  TableHead,
  TableBody,
  TableCell,
} from "../../components/ui/table";
import { Empty, ErrorNotice, Loading, PageHeading } from "../library/shared";
import { tagFields } from "./tag-model";
import { useTagEditor } from "./use-tag-editor";
import "./translations";

function Albums({ albums }: { albums: MusicAlbumView[] }) {
  const { t } = useTranslation();
  if (!albums.length) return <Empty title={t("musicEmpty")} />;
  return (
    <div className="grid gap-3 md:grid-cols-2">
      {albums.map((album) => (
        <Card asChild key={album.release_id}>
          <Anchor
            className="block"
            href={`/music/album/${encodeURIComponent(album.release_id)}`}
          >
            <h2 className="font-semibold">{album.title}</h2>
            <p>{album.artist}</p>
            <p className="text-sm text-muted-foreground">
              {album.country ?? t("musicUnknown")} ·{" "}
              {album.date ?? t("musicUnknown")} · {t("musicDiscs")}:{" "}
              {album.medium_count} · {t("musicTracks")}: {album.track_count}
            </p>
          </Anchor>
        </Card>
      ))}
    </div>
  );
}
export function MusicPage() {
  const { t } = useTranslation();
  const albums = useQuery({
    queryKey: ["music-library"],
    queryFn: musicApi.library,
  });
  return (
    <div className="space-y-6">
      <PageHeading title={t("music")}>
        <Anchor href="/music/tools" className="underline">
          {t("musicTools")}
        </Anchor>
      </PageHeading>
      {albums.isPending ? (
        <Loading />
      ) : albums.error ? (
        <ErrorNotice error={albums.error} retry={() => void albums.refetch()} />
      ) : (
        <Albums albums={albums.data ?? []} />
      )}
    </div>
  );
}
export function MusicToolsPage() {
  const { t } = useTranslation();
  const client = useQueryClient();
  const albums = useQuery({
    queryKey: ["music-tools-library"],
    queryFn: musicApi.toolsLibrary,
  });
  const [albumId, setAlbumId] = useState("");
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState<MusicReleaseGraph>();
  const [selectedMbid, setSelectedMbid] = useState("");
  const search = useMutation({
    mutationFn: musicApi.search,
    onSuccess: () => setSelected(undefined),
  });
  const detail = useMutation({
    mutationFn: musicApi.release,
    onSuccess: (graph, mbid) => {
      setSelected(graph);
      setSelectedMbid(mbid);
    },
  });
  const add = useMutation({
    mutationFn: musicApi.import,
    onSuccess: (graph) => {
      setSelected(graph);
      setAlbumId(graph.release.media_item_id);
      void client.invalidateQueries({ queryKey: ["music-library"] });
      void client.invalidateQueries({ queryKey: ["music-tools-library"] });
    },
  });
  return (
    <div className="space-y-6">
      <PageHeading title={t("musicTools")} />
      <Card asChild>
        <section className="space-y-4">
          <h2 className="font-semibold">{t("musicSearch")}</h2>
          <form
            className="flex flex-wrap gap-3"
            onSubmit={(e) => {
              e.preventDefault();
              if (query.trim()) {
                setSelected(undefined);
                detail.reset();
                add.reset();
                search.mutate(query.trim());
              }
            }}
          >
            <Input
              aria-label={t("musicSearchHint")}
              placeholder={t("musicSearchHint")}
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              maxLength={500}
            />
            <Button
              type="submit"
              disabled={
                !query.trim() ||
                search.isPending ||
                detail.isPending ||
                add.isPending
              }
            >
              {t("musicSearch")}
            </Button>
          </form>
          {search.isPending && <Loading />}
          {search.error && <ErrorNotice error={search.error} />}
          {search.data && (
            <div className="space-y-2">
              <h3>{t("musicEditions")}</h3>
              {search.data.length === 0 && <Empty title={t("noResults")} />}{" "}
              {search.data.map((c) => (
                <Card
                  key={c.mbid}
                  className="flex flex-wrap items-center justify-between gap-3 rounded-lg p-3 sm:p-3"
                >
                  <div>
                    <p className="font-medium">
                      {c.title} — {c.artist}
                    </p>
                    <p>
                      {c.country ?? t("musicUnknown")} ·{" "}
                      {c.date ?? t("musicUnknown")} ·{" "}
                      {c.disambiguation ?? t("musicUnknown")} · {t("musicDiscs")}:{" "}
                      {c.medium_count} · {t("musicTracks")}: {c.track_count}
                    </p>
                    <small>MBID: {c.mbid}</small>
                  </div>
                  <Button
                    variant="outline"
                    disabled={
                      search.isPending || detail.isPending || add.isPending
                    }
                    onClick={() => {
                      setSelected(undefined);
                      add.reset();
                      detail.mutate(c.mbid);
                    }}
                  >
                    {t("musicSelect")}
                  </Button>
                </Card>
              ))}
            </div>
          )}
          {detail.isPending && <Loading />}
          {detail.error && <ErrorNotice error={detail.error} />}
          {selected && (
            <section aria-label={t("musicEdition")} className="space-y-3">
              <ReleaseContents graph={selected} />
              <Button
                disabled={add.isPending || add.isSuccess}
                onClick={() =>
                  add.mutate(selectedMbid, {
                    onSuccess: () => toast.success(t("musicImported")),
                  })
                }
              >
                {t("musicImport")}
              </Button>
            </section>
          )}
          {add.error && <ErrorNotice error={add.error} />}
        </section>
      </Card>
      {albums.isPending ? (
        <Loading />
      ) : albums.error ? (
        <ErrorNotice error={albums.error} retry={() => void albums.refetch()} />
      ) : (
        <Card asChild>
          <section className="space-y-4">
            <Label className="block">
              {t("musicSelectAlbum")}
              <Select
                className="ml-3"
                aria-label={t("musicSelectAlbum")}
                value={albumId}
                onChange={(event) => setAlbumId(event.target.value)}
              >
                <option value="">{t("musicSelectAlbum")}</option>
                {albums.data?.map((album) => (
                  <option key={album.release_id} value={album.release_id}>
                    {album.title} · {album.artist} · {album.country ?? "—"} ·{" "}
                    {album.date ?? "—"}
                  </option>
                ))}
              </Select>
            </Label>
            {albumId && <MusicAlbumTools key={albumId} releaseId={albumId} />}
          </section>
        </Card>
      )}
    </div>
  );
}
function ReleaseContents({ graph }: { graph: MusicReleaseGraph }) {
  const { t, i18n } = useTranslation();
  const source = graph.source_url;
  return (
    <div className="space-y-4">
      <h2 className="text-xl font-semibold">{graph.release.title}</h2>
      <p>
        {graph.release.country ?? t("musicUnknown")} ·{" "}
        {graph.release_date_text ??
          graph.release.release_date ??
          t("musicUnknown")}{" "}
        · {graph.release.status ?? t("musicUnknown")} ·{" "}
        {graph.release_disambiguation ?? ""}
      </p>
      <p>
        {t("musicArtist")}:{" "}
        {graph.artists.map((a) => (
          <Anchor
            className="mr-2 underline"
            key={a.media_item_id}
            href={`/music/artist/${encodeURIComponent(a.media_item_id)}`}
          >
            {a.name}
          </Anchor>
        ))}
      </p>
      <p className="text-sm">
        {t("musicSource")}:{" "}
        <Anchor
          className="underline"
          href={source}
          rel="noreferrer"
          target="_blank"
        >
          MusicBrainz
        </Anchor>{" "}
        · {t("musicLanguage")}: {graph.locale ?? t("musicUnknown")} ·{" "}
        {t("musicFetched")}:{" "}
        {new Intl.DateTimeFormat(i18n.resolvedLanguage, {
          dateStyle: "medium",
          timeStyle: "short",
        }).format(new Date(graph.fetched_at))}
      </p>
      {[...graph.media]
        .sort((a, b) => a.position - b.position)
        .map((medium) => (
          <section key={medium.id}>
            <h3 className="mb-2 font-semibold">
              {t("musicDisc", { number: medium.position })} {medium.title}{" "}
              {medium.format}
            </h3>
            <div className="overflow-x-auto">
              <Table className="w-full text-left text-sm">
                <TableHeader>
                  <TableRow>
                    <TableHead>{t("musicTrackNumber")}</TableHead>
                    <TableHead>{t("musicField_title")}</TableHead>
                    <TableHead>{t("musicArtist")}</TableHead>
                    <TableHead>{t("musicLength")}</TableHead>
                    <TableHead>{t("musicRecording")}</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {graph.tracks
                    .filter((track) => track.medium_id === medium.id)
                    .sort((a, b) => a.position - b.position)
                    .map((track) => (
                      <TableRow key={track.media_item_id}>
                        <TableCell className="py-2">{track.number}</TableCell>
                        <TableCell>{track.title}</TableCell>
                        <TableCell>
                          {track.artist_credits
                            .map((c) => `${c.credited_name}${c.join_phrase}`)
                            .join("")}
                        </TableCell>
                        <TableCell>
                          {track.length_ms === null
                            ? "—"
                            : `${Math.floor(track.length_ms / 60000)}:${String(Math.floor(track.length_ms / 1000) % 60).padStart(2, "0")}`}
                        </TableCell>
                        <TableCell>
                          <small>
                            {graph.identifiers.find(
                              (id) =>
                                id.media_item_id === track.recording_id &&
                                id.kind === "music_brainz",
                            )?.value ??
                              graph.recordings.find(
                                (recording) =>
                                  recording.media_item_id ===
                                  track.recording_id,
                              )?.title ??
                              t("musicUnknown")}
                          </small>
                        </TableCell>
                      </TableRow>
                    ))}
                </TableBody>
              </Table>
            </div>
          </section>
        ))}
      <Details>
        <DetailsSummary>{t("musicIdentifiers")}</DetailsSummary>
        {graph.identifiers.map((id) => (
          <p key={`${id.media_item_id}-${id.kind}-${id.namespace}-${id.value}`}>
            {id.namespace} / {id.kind}: {id.value}
          </p>
        ))}
      </Details>
    </div>
  );
}
export function MusicArtistPage({ artistId }: { artistId: string }) {
  const { t } = useTranslation();
  const query = useQuery({
    queryKey: ["music-artist", artistId],
    queryFn: () => musicApi.artist(artistId),
  });
  if (query.isPending) return <Loading />;
  if (query.error) return <ErrorNotice error={query.error} />;
  if (!query.data) return null;
  return (
    <div className="space-y-5">
      <Anchor className="underline" href="/music">
        {t("musicBack")}
      </Anchor>
      <PageHeading
        title={query.data.artist.name}
        description={query.data.artist.sort_name ?? undefined}
      />
      <p>
        {t("musicAliases")}: {query.data.artist.aliases.join(" · ") || "—"}
      </p>
      <Albums albums={query.data.releases} />
    </div>
  );
}
export function MusicAlbumPage({ releaseId }: { releaseId: string }) {
  const { t } = useTranslation();
  const album = useQuery({
    queryKey: ["music-album", releaseId],
    queryFn: () => musicApi.album(releaseId),
  });
  const cover = useQuery({
    queryKey: ["music-cached-cover", releaseId],
    queryFn: () => musicApi.cachedCover(releaseId),
  });
  if (album.isPending) return <Loading />;
  if (album.error) return <ErrorNotice error={album.error} />;
  if (!album.data) return null;
  const graph = album.data;
  return (
    <div className="space-y-6">
      <Anchor className="underline" href="/music">
        {t("musicBack")}
      </Anchor>
      <Card asChild>
        <section>
          <ReleaseContents graph={graph} />
        </section>
      </Card>
      <Card asChild>
        <section className="space-y-3">
          <h2>{t("musicArtwork")}</h2>
          {cover.isPending ? (
            <Loading />
          ) : cover.error ? (
            <ErrorNotice error={cover.error} />
          ) : cover.data ? (
            <CoverFigure cover={cover.data} title={graph.release.title} />
          ) : (
            <p>{t("musicNoCover")}</p>
          )}
        </section>
      </Card>
      <Card asChild>
        <section className="space-y-3">
          <h2 className="font-semibold">{t("musicFieldSources")}</h2>
          {graph.field_sources?.length ? (
            <div className="overflow-x-auto">
              <Table className="w-full text-left text-sm">
                <TableHeader>
                  <TableRow>
                    <TableHead>{t("musicEntity")}</TableHead>
                    <TableHead>{t("musicField")}</TableHead>
                    <TableHead>{t("musicLanguage")}</TableHead>
                    <TableHead>{t("musicAfter")}</TableHead>
                    <TableHead>{t("musicSource")}</TableHead>
                    <TableHead>{t("musicProtection")}</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {graph.field_sources.map((field) => (
                    <TableRow key={field.id}>
                      <TableCell className="py-2">
                        {graph.artists.find(
                          (a) => a.media_item_id === field.media_item_id,
                        )?.name ??
                          graph.tracks.find(
                            (tr) => tr.media_item_id === field.media_item_id,
                          )?.title ??
                          graph.recordings.find(
                            (r) => r.media_item_id === field.media_item_id,
                          )?.title ??
                          (field.media_item_id === graph.release.media_item_id
                            ? graph.release.title
                            : graph.release_group.title)}
                      </TableCell>
                      <TableCell>
                        {t(`musicField_${field.field_key}`, {
                          defaultValue: field.field_key,
                        })}
                      </TableCell>
                      <TableCell>{field.locale ?? "—"}</TableCell>
                      <TableCell>
                        {typeof field.value === "string"
                          ? field.value
                          : JSON.stringify(field.value)}
                      </TableCell>
                      <TableCell>
                        {field.provider_id ?? t("musicManual")}
                        {field.source_url && (
                          <Anchor
                            href={field.source_url}
                            className="ml-2 underline"
                            target="_blank"
                            rel="noreferrer"
                          >
                            {t("musicSource")}
                          </Anchor>
                        )}
                      </TableCell>
                      <TableCell>
                        {t(
                          field.is_locked
                            ? "musicLocked"
                            : field.is_user_modified
                              ? "musicUserModified"
                              : "musicProviderValue",
                        )}
                      </TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            </div>
          ) : (
            <p>{t("musicNoSources")}</p>
          )}
        </section>
      </Card>
      <Card asChild>
        <section className="space-y-3">
          <h2 className="font-semibold">{t("musicLocalFile")}</h2>
          <LocalAudioFiles graph={graph} />
        </section>
      </Card>
      <Anchor className="underline" href="/music/tools">
        {t("musicTools")}
      </Anchor>
    </div>
  );
}
function LocalAudioFiles({ graph }: { graph: MusicReleaseGraph }) {
  const { t } = useTranslation();
  const open = useMutation({ mutationFn: musicApi.openFile });
  return (
    <div className="space-y-3">
      {graph.files.length ? (
        graph.files.map((file) => (
          <div key={file.file_id} className="flex flex-wrap items-center gap-3">
            <span>{file.relative_path}</span>
            <Button
              variant="outline"
              disabled={open.isPending}
              onClick={() =>
                open.mutate(file.file_id, {
                  onSuccess: (result) => {
                    if (result.requested)
                      toast.success(t("musicOpenRequested"));
                  },
                })
              }
            >
              {t("musicOpenNative")}
            </Button>
          </div>
        ))
      ) : (
        <p>{t("musicNoFiles")}</p>
      )}
      {open.error && <ErrorNotice error={open.error} />}
    </div>
  );
}
export function MusicAlbumTools({ releaseId }: { releaseId: string }) {
  const { t } = useTranslation();
  const album = useQuery({
    queryKey: ["music-tools-album", releaseId],
    queryFn: () => musicApi.toolsAlbum(releaseId),
  });
  const [file, setFile] = useState("");
  if (album.isPending) return <Loading />;
  if (album.error) return <ErrorNotice error={album.error} />;
  if (!album.data) return null;
  const graph = album.data;
  return (
    <div className="space-y-6">
      {" "}
      <MusicCoverTools graph={graph} />
      <LocalLink graph={graph} />
      <Card asChild>
        <section className="space-y-4">
          <h2 className="text-lg font-semibold">{t("musicTags")}</h2>
          {graph.files.length ? (
            <Label className="block">
              {t("musicLocalFile")}
              <Select
                className="ml-3"
                value={file}
                onChange={(e) => setFile(e.target.value)}
              >
                <option value="">{t("musicPickFile")}</option>
                {graph.files.map((f) => (
                  <option key={f.file_id} value={f.file_id}>
                    {f.relative_path}
                  </option>
                ))}
              </Select>
            </Label>
          ) : (
            <p>{t("musicNoFiles")}</p>
          )}
          {file && graph.files.some((f) => f.file_id === file) && (
            <TagEditor key={file} fileId={file} graph={graph} />
          )}
        </section>
      </Card>
    </div>
  );
}
function CoverFigure({
  cover,
  title,
}: {
  cover: MusicCoverPreview;
  title: string;
}) {
  const { t } = useTranslation();
  return (
    <figure>
      <img src={cover.thumbnail} alt={title} width={240} />
      <figcaption>
        {t("musicSource")}: {cover.source_url} · {t("musicRights")}:{" "}
        {cover.license}
      </figcaption>
    </figure>
  );
}
function MusicCoverTools({ graph }: { graph: MusicReleaseGraph }) {
  const { t } = useTranslation();
  const client = useQueryClient();
  const releaseId = graph.release.media_item_id;
  const mbid = graph.identifiers.find(
    (id) => id.media_item_id === releaseId && id.kind === "music_brainz",
  )?.value;
  const cover = useMutation({
    mutationFn: () => musicApi.coverPreview(mbid ?? "", 0, releaseId),
    onSuccess: () =>
      void client.invalidateQueries({
        queryKey: ["music-cached-cover", releaseId],
      }),
  });
  if (!mbid) return null;
  return (
    <Card asChild>
      <section className="space-y-3">
        <h2>{t("musicArtwork")}</h2>
        <Button
          variant="outline"
          disabled={cover.isPending}
          onClick={() => cover.mutate()}
        >
          {t("musicLoadCover")}
        </Button>
        {cover.data && (
          <CoverFigure cover={cover.data} title={graph.release.title} />
        )}{" "}
        {cover.error && <ErrorNotice error={cover.error} />}
      </section>
    </Card>
  );
}
function LocalLink({ graph }: { graph: MusicReleaseGraph }) {
  const { t } = useTranslation();
  const client = useQueryClient();
  const [track, setTrack] = useState("");
  const [file, setFile] = useState("");
  const local = useQuery({
    queryKey: ["music-local-files"],
    queryFn: musicApi.localFiles,
  });
  const link = useMutation({
    mutationFn: () => musicApi.linkTrack(track, file),
    onSuccess: () => {
      setFile("");
      void client.invalidateQueries({
        queryKey: ["music-album", graph.release.media_item_id],
      });
      void client.invalidateQueries({
        queryKey: ["music-tools-album", graph.release.media_item_id],
      });
      void client.invalidateQueries({ queryKey: ["music-library"] });
    },
  });
  return (
    <Card asChild>
      <section className="space-y-3">
        <p>{t("musicLinkHelp")}</p>
        <div className="flex flex-wrap gap-3">
          <Select
            aria-label={t("musicChooseTrack")}
            value={track}
            onChange={(e) => setTrack(e.target.value)}
            disabled={link.isPending}
          >
            <option value="">{t("musicChooseTrack")}</option>
            {graph.tracks.map((tr) => (
              <option key={tr.media_item_id} value={tr.media_item_id}>
                {graph.media.find((m) => m.id === tr.medium_id)?.position}-
                {tr.number} {tr.title}
              </option>
            ))}
          </Select>
          <Select
            aria-label={t("musicChooseFile")}
            value={file}
            onChange={(e) => setFile(e.target.value)}
            disabled={local.isPending || link.isPending}
          >
            <option value="">{t("musicChooseFile")}</option>
            {local.data?.map((f) => (
              <option key={f.file_id} value={f.file_id}>
                {f.relative_path}
              </option>
            ))}
          </Select>
          <Button
            disabled={!track || !file || link.isPending}
            onClick={() => link.mutate()}
          >
            {t("musicLink")}
          </Button>
        </div>
        {[local.error, link.error].filter(Boolean).map((e, i) => (
          <ErrorNotice key={String(i)} error={e} />
        ))}
      </section>
    </Card>
  );
}
export function TagEditor({
  fileId,
  graph,
}: {
  fileId: string;
  graph: MusicReleaseGraph;
}) {
  const { t } = useTranslation();
  const {
    tags,
    history,
    values,
    setValues,
    selected,
    setSelected,
    preview,
    setPreview,
    restore,
    confirmed,
    setConfirmed,
    operation,
    plan,
    write,
    restorePlan,
    busy,
  } = useTagEditor(fileId, graph);
  return (
    <div className="space-y-4">
      {tags.data && (
        <p>
          {t("musicFormat")}: {tags.data.format} ·{" "}
          {t(tags.data.writable ? "musicWritable" : "musicReadOnly")} ·{" "}
          {t("musicPictureCount", { count: tags.data.picture_count })}
        </p>
      )}
      {tags.isPending && <Loading />}
      {tags.error && <ErrorNotice error={tags.error} />}
      {tags.data && (
        <>
          <div className="overflow-x-auto">
            <Table className="w-full text-left text-sm">
              <TableHeader>
                <TableRow>
                  <TableHead>{t("musicField")}</TableHead>
                  <TableHead>{t("musicBefore")}</TableHead>
                  <TableHead>{t("musicAfter")}</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {tagFields.map((f) => (
                  <TableRow key={f}>
                    <TableCell>
                      <Label>
                        <Checkbox
                          aria-label={t("musicSelectField", {
                            field: t(`musicField_${f}`),
                          })}
                          checked={selected.includes(f)}
                          disabled={busy || Boolean(preview)}
                          onChange={(e) =>
                            setSelected((old) =>
                              e.target.checked
                                ? [...old, f]
                                : old.filter((v) => v !== f),
                            )
                          }
                        />
                        {t(`musicField_${f}`)}
                      </Label>
                    </TableCell>
                    <TableCell className="p-2">{tags.data.tags[f] ?? "—"}</TableCell>
                    <TableCell>
                      <Input
                        aria-label={t(`musicField_${f}`)}
                        value={values[f]}
                        disabled={busy || Boolean(preview)}
                        maxLength={4096}
                        onChange={(e) =>
                          setValues({ ...values, [f]: e.target.value })
                        }
                      />
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </div>
          <Button
            disabled={
              !tags.data.writable ||
              !selected.length ||
              busy ||
              Boolean(preview)
            }
            onClick={() => {
              write.reset();
              plan.mutate();
            }}
          >
            {t("musicPreview")}
          </Button>
        </>
      )}
      {preview && (
        <Card asChild className="space-y-3 rounded-lg p-4 sm:p-4">
          <section aria-label={t("musicPreview")}>
            <Table className="w-full text-left text-sm">
              <TableHeader>
                <TableRow>
                  <TableHead>{t("musicField")}</TableHead>
                  <TableHead>{t("musicBefore")}</TableHead>
                  <TableHead>{t("musicAfter")}</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {preview.changes.map((c) => (
                  <TableRow key={c.field}>
                    <TableCell>
                      {t(`musicField_${c.field}`, { defaultValue: c.field })}
                    </TableCell>
                    <TableCell>{c.before ?? "—"}</TableCell>
                    <TableCell>{c.after ?? "—"}</TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
            <Label className="block">
              <Checkbox
                checked={confirmed}
                disabled={busy}
                onChange={(e) => setConfirmed(e.target.checked)}
              />
              {t(restore ? "musicRestoreConfirm" : "musicWriteConfirm")}
            </Label>
            <div className="flex gap-3">
              <Button
                disabled={!confirmed || busy}
                onClick={() => write.mutate()}
              >
                {t(restore ? "musicRestoreExecute" : "musicWrite")}
              </Button>
              <Button
                variant="outline"
                disabled={busy}
                onClick={() => {
                  setPreview(undefined);
                  setConfirmed(false);
                }}
              >
                {t("musicCancel")}
              </Button>
            </div>
          </section>
        </Card>
      )}
      {operation && (
        <div className="space-y-3">
          <Badge
            variant={
              operation.state === "succeeded" ? "secondary" : "destructive"
            }
          >
            {t(`musicState_${operation.state}`, {
              defaultValue: t("musicUnknown"),
            })}
          </Badge>
          <Button
            variant="outline"
            disabled={
              busy || Boolean(preview) || operation.backup_relative === null
            }
            onClick={() => restorePlan.mutate(operation.operation_id)}
          >
            {t("musicRestore")}
          </Button>
        </div>
      )}
      <section className="space-y-2">
        <h3>{t("musicHistory")}</h3>
        {history.data
          ?.filter(
            (entry) =>
              entry.backup_relative !== null &&
              entry.operation_id !== operation?.operation_id,
          )
          .map((entry) => (
            <div
              key={entry.operation_id}
              className="flex flex-wrap items-center gap-3"
            >
              <span>
                {t("musicSavedOperation")}: {entry.operation_id} ·{" "}
                {t(`musicState_${entry.state}`, {
                  defaultValue: t("musicUnknown"),
                })}
              </span>
              <Button
                variant="outline"
                disabled={busy || Boolean(preview)}
                onClick={() => restorePlan.mutate(entry.operation_id)}
              >
                {t("musicRestore")}
              </Button>
            </div>
          ))}
        {history.error && <ErrorNotice error={history.error} />}
      </section>
      {[plan.error, write.error, restorePlan.error]
        .filter(Boolean)
        .map((e, i) => (
          <ErrorNotice key={String(i)} error={e} />
        ))}
    </div>
  );
}
