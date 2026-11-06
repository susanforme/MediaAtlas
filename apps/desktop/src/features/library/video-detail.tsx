import { useQuery } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { FileVideo, Film, Wrench } from "lucide-react";
import { type ReactNode, useState } from "react";
import { useTranslation } from "react-i18next";
import type { MediaDetail } from "../../api/library";
import { scraperApi, type SeasonDetail } from "../../api/scraper";
import { Badge } from "../../components/ui/badge";
import { Button } from "../../components/ui/button";
import { Card } from "../../components/ui/card";
import { BackNavigation } from "../../components/shell/back-navigation";
import { VideoSourceDetails } from "../scraper/source-detail";
import { DetailRecoveryAction } from "./detail-recovery-action";
import { MediaDetailSections, MediaDetailView } from "./media-detail-view";
import { MediaCast, MediaMetadataFields } from "./media-metadata";
import { NativeOpen, playableFiles } from "./native-open";
import { ErrorNotice, HealthBadges, useFormat } from "./shared";
import { videoDetailFields } from "./video-detail-model";
import "./video-detail-translations";

function VideoSeasons({ seasons }: { seasons: SeasonDetail[] }) {
  const { t } = useTranslation();
  const { number } = useFormat();
  const [selected, setSelected] = useState<number>();
  const season =
    seasons.find((item) => item.season_number === selected) ?? seasons[0];
  if (!season)
    return (
      <p className="text-muted-foreground">{t("videoDetailNoSeasons")}</p>
    );
  return (
    <div>
      <div
        className="flex gap-3 overflow-x-auto p-[3px]"
        role="group"
        aria-label={t("videoDetailSeasons")}
      >
        {seasons.map((item) => (
          <Button
            key={`${item.season_number}:${item.remote_id}`}
            variant="ghost"
            className="flex h-auto min-h-[150px] w-[148px] flex-col items-start justify-end gap-2 border border-border bg-surface px-3 py-4 text-left whitespace-normal text-foreground [&:hover]:bg-surface aria-pressed:border-ring aria-pressed:bg-muted [&[aria-pressed=true]:hover]:bg-muted [&>svg]:mb-auto [&>svg]:text-muted-foreground"
            aria-pressed={item === season}
            onClick={() => setSelected(item.season_number)}
          >
            <Film size={30} aria-hidden="true" />
            <span>
              {t("videoDetailSeason", {
                number: number(item.season_number),
              })}
            </span>
            <span className="text-[12px] leading-[var(--text-sm--line-height)] font-normal text-muted-foreground">
              {t("videoDetailSourceEpisodes", { count: item.episodes.length })}
            </span>
          </Button>
        ))}
      </div>
      <h3 className="mt-6 mb-3 text-[18px] font-semibold">
        {season.title ||
          t("videoDetailSeason", { number: number(season.season_number) })}
      </h3>
      <ol className="m-0 list-none p-0 [&_h4]:text-[14px] [&_h4]:font-medium [&_p]:mt-1.5 [&_p]:text-[13px] [&_p]:leading-[1.7] [&_p]:text-muted-foreground">
        {season.episodes.map((episode) => (
          <Card asChild key={`${episode.episode_number}:${episode.remote_id}`} className="mt-2 flex items-baseline gap-[18px] p-[18px] sm:p-[18px]">
            <li>
              <span className="text-ring tabular-nums">
                {number(episode.episode_number)}
              </span>
              <div>
                <h4>
                  {episode.title ||
                    t("videoDetailEpisode", {
                      number: number(episode.episode_number),
                    })}
                </h4>
                {episode.overview && <p>{episode.overview}</p>}
              </div>
            </li>
          </Card>
        ))}
      </ol>
      {!season.episodes.length && (
        <p className="text-muted-foreground">{t("videoDetailNoEpisodes")}</p>
      )}
    </div>
  );
}

export function VideoDetail({
  detail,
  sourceDetail,
}: {
  detail: MediaDetail;
  sourceDetail?: ReactNode;
}) {
  const { t, i18n } = useTranslation();
  const { number } = useFormat();
  const isSeries = detail.media.kind === "tv_series";
  const source = useQuery({
    queryKey: ["video-source-detail", detail.media.id],
    queryFn: () => scraperApi.mediaDetail(detail.media.id),
  });
  const fields = videoDetailFields(detail, i18n.resolvedLanguage ?? "zh-CN");
  const missingNfo = detail.health.includes("missing_nfo");
  const failedNfo = detail.health.includes("nfo_output_failed");
  const playable = playableFiles(detail.files);
  const visibleHealth = detail.health.filter(
    (health) =>
      !["metadata_pending", "missing_nfo", "nfo_output_failed"].includes(health),
  );
  const facts = [
    { key: "directors", label: t("videoDetailDirector"), value: fields.directors.length ? fields.directors.join(" / ") : null },
    { key: "cast", label: t("videoDetailCast"), value: fields.cast.length ? fields.cast.slice(0, 4).join(" / ") : null },
    { key: "premiered", label: t("videoDetailPremiere"), value: fields.premiered || null },
    { key: "language", label: t("language"), value: fields.language || null },
    { key: "status", label: t("scraperField_status"), value: fields.status || null },
    { key: "genres", label: t("scraperField_genres"), value: fields.genres.length ? <span className="flex flex-wrap gap-1.5">{fields.genres.map((genre) => <Badge key={genre} variant="secondary">{genre}</Badge>)}</span> : null },
  ].filter((fact) => fact.value !== null);
  return (
    <MediaDetailView
      title={detail.media.title}
      originalTitle={detail.media.original_title}
      year={detail.media.year}
      kind={t(`kind_${detail.media.kind}`)}
      runtime={fields.runtime}
      genres={fields.genres}
      rating={fields.rating}
      poster={{ uri: detail.poster_uri, updatedAt: detail.media.updated_at }}
      overview={fields.overview}
      facts={facts}
      navigation={<BackNavigation />}
      actions={
        <>
            <NativeOpen files={detail.files} />
            <DetailRecoveryAction detail={detail} />
            <Button asChild variant="outline">
              <Link to="/tools" search={{ media: detail.media.id }}>
                <Wrench size={17} aria-hidden="true" />
                {t("videoTools")}
              </Link>
            </Button>
        </>
      }
      status={
            <Card asChild className="p-5 sm:p-5">
              <aside
                aria-label={t("videoDetailStatus")}
              >
                <h2 data-slot="video-status-heading" className="mb-3 text-[16px] font-semibold">{t("videoDetailStatus")}</h2>
                <HealthBadges health={visibleHealth} />
                <dl className="mt-5 grid gap-4 text-[13px] [&>div]:grid [&>div]:grid-cols-2 [&>div]:gap-3 [&_dt]:text-muted-foreground [&_dd]:text-right [&_dd]:[overflow-wrap:anywhere] [@media(width<=1350px)]:grid-cols-[repeat(auto-fit,minmax(180px,1fr))]">
                  <div>
                    <dt>{t("metadata")}</dt>
                    <dd>
                      {detail.metadata_values.length
                        ? t("videoDetailFields", {
                            count: detail.metadata_values.length,
                          })
                        : t("noMetadata")}
                    </dd>
                  </div>
                  <div>
                    <dt>{t("videoDetailPoster")}</dt>
                    <dd>
                      {t(detail.poster_uri ? "videoDetailAvailable" : "videoDetailMissing")}
                    </dd>
                  </div>
                  {detail.external_ids.map((id) => (
                    <div key={id.id}>
                      <dt>{id.provider_id} / {id.namespace}</dt>
                      <dd>{id.external_id}</dd>
                    </div>
                  ))}
                </dl>
                {(missingNfo || failedNfo) && (
                  <section
                    className="mt-5 border-t border-border pt-4 [&_p]:mt-2.5 [&_p]:mb-3 [&_p]:text-[12px] [&_p]:leading-[1.6] [&_p]:text-muted-foreground"
                    aria-label={t("libraryNfoOutput")}
                  >
                    <Badge variant={failedNfo ? "destructive" : "secondary"}>
                      {t(failedNfo ? "health_nfo_output_failed" : "health_missing_nfo")}
                    </Badge>
                    <p>
                      {t(failedNfo ? "libraryNfoFailedHint" : "libraryNfoPendingHint")}
                    </p>
                    <Button asChild variant="outline" size="sm">
                      <Link to="/roots" search={{}}>{t("libraryRefreshManage")}</Link>
                    </Button>
                  </section>
                )}
              </aside>
            </Card>
      }
    >
          {sourceDetail}
          <MediaDetailSections initialTab={isSeries ? "seasons" : "files"} sections={{
            files: <>
              {detail.files.length ? (
                <ul className="m-0 list-none p-0">
                  {detail.files.map((file) => (
                    <Card asChild key={file.id} className="mb-3 flex items-center gap-4 p-5 sm:p-5 [&>svg]:shrink-0 [&>svg]:text-ring [@media(width<=760px)]:flex-wrap [@media(width<=760px)]:p-4!">
                      <li>
                        <FileVideo size={25} aria-hidden="true" />
                        <div className="min-w-0 flex-1 [@media(width<=760px)]:basis-[calc(100%-48px)]">
                          <h3 className="text-[14px] font-semibold [overflow-wrap:anywhere]">{file.relative_path.split(/[\\/]/).at(-1)}</h3>
                          <p className="mt-[5px] text-[12px] text-muted-foreground [overflow-wrap:anywhere]">{file.relative_path}</p>
                        </div>
                        <span className="m-0 shrink-0 text-[12px] text-muted-foreground [overflow-wrap:anywhere]">
                          {number(file.size_bytes)} B
                          {file.part !== null && ` · ${t("part")} ${number(file.part)}`}
                        </span>
                        {playable.some((item) => item.id === file.id) && (
                          <NativeOpen
                            files={[file]}
                            label={file.relative_path}
                            className="min-w-0 px-3 text-[12px] leading-[var(--text-sm--line-height)]"
                          />
                        )}
                      </li>
                    </Card>
                  ))}
                </ul>
              ) : (
                <p className="text-muted-foreground">{t("noFiles")}</p>
              )}
            </>,
            seasons: isSeries ? (
              <div aria-busy={source.isFetching}>
                {source.isPending ? (
                  <p className="text-muted-foreground" role="status">
                    {t("loading")}
                  </p>
                ) : source.isError ? (
                  <ErrorNotice error={source.error} retry={() => void source.refetch()} />
                ) : (
                  <VideoSeasons
                    seasons={source.data?.kind === "tv" ? source.data.data.seasons : []}
                  />
                )}
              </div>
            ) : undefined,
            cast: fields.cast.length ? <MediaCast names={fields.cast} /> : undefined,
            metadata: <>
              {detail.metadata_values.length ? (
                <MediaMetadataFields fields={detail.metadata_values.map((field) => ({
                  key: field.id, field: field.field_key,
                  label: t(`scraperField_${field.field_key}`, { defaultValue: field.field_key }),
                  value: field.value, locale: field.locale, locked: field.is_locked,
                  edited: field.is_user_modified, provider: field.provider_id,
                }))} />
              ) : (
                <p className="text-muted-foreground">{t("noMetadata")}</p>
              )}
              <section className="mt-6 border-t border-border pt-5">
                <h3 className="mb-3 text-[16px] font-semibold">{t("scraperLibrarySourceTitle")}</h3>
                {source.isPending ? (
                  <p className="text-muted-foreground" role="status">
                    {t("loading")}
                  </p>
                ) : source.isError ? (
                  <ErrorNotice error={source.error} retry={() => void source.refetch()} />
                ) : source.data ? (
                  <VideoSourceDetails metadata={source.data} />
                ) : (
                  <p className="text-muted-foreground">{t("scraperLibrarySourceEmpty")}</p>
                )}
              </section>
            </>,
            related: detail.relations.length > 0 ? (
                <ul className="m-0 grid list-none gap-2.5 p-0 [&_a]:flex [&_a]:h-auto [&_a]:justify-between [&_a]:p-4 [&_a]:whitespace-normal [&_span]:text-[12px] [&_span]:text-muted-foreground">
                  {detail.relations.map((item) => (
                    <li key={item.id}>
                      <Button asChild variant="secondary">
                        <Link
                          to="/media/$mediaId"
                          params={{ mediaId: item.id }}
                          search={{}}
                        >
                          {item.title}<span>{t(`kind_${item.kind}`)}</span>
                        </Link>
                      </Button>
                    </li>
                  ))}
                </ul>
            ) : undefined,
            artwork: detail.artworks.length > 0 ? (
                <ul className="m-0 grid list-none gap-2.5 p-0">
                  {detail.artworks.map((art) => (
                    <Card asChild key={art.id} className="flex items-center justify-between gap-3 p-4 sm:p-4">
                      <li>
                        <span>{art.kind}</span>
                        <Badge variant="secondary">
                          {t(art.cache_path ? "videoDetailAvailable" : "videoDetailMissing")}
                        </Badge>
                      </li>
                    </Card>
                  ))}
                </ul>
            ) : undefined,
          }} />
    </MediaDetailView>
  );
}
