import { useQuery } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { ArrowLeft, FileVideo, Film, LockKeyhole, Star, Wrench } from "lucide-react";
import { type ReactNode, useState } from "react";
import { useTranslation } from "react-i18next";
import type { MediaDetail } from "../../api/library";
import { scraperApi, type SeasonDetail } from "../../api/scraper";
import { Badge } from "../../components/ui/badge";
import { Button } from "../../components/ui/button";
import { Card } from "../../components/ui/card";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "../../components/ui/tabs";
import { VideoSourceDetails } from "../scraper/source-detail";
import { DetailRecoveryAction } from "./detail-recovery-action";
import { MediaArtwork } from "./media-artwork";
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
  const { number, date } = useFormat();
  const isSeries = detail.media.kind === "tv_series";
  const [tab, setTab] = useState(isSeries ? "seasons" : "files");
  const [expanded, setExpanded] = useState(false);
  const source = useQuery({
    queryKey: ["video-source-detail", detail.media.id],
    queryFn: () => scraperApi.mediaDetail(detail.media.id),
  });
  const fields = videoDetailFields(detail, i18n.resolvedLanguage ?? "zh-CN");
  const canExpand = fields.overview.length > 240;
  const missingNfo = detail.health.includes("missing_nfo");
  const failedNfo = detail.health.includes("nfo_output_failed");
  const playable = playableFiles(detail.files);
  const visibleHealth = detail.health.filter(
    (health) =>
      !["metadata_pending", "missing_nfo", "nfo_output_failed"].includes(health),
  );
  return (
    <article className="min-w-0 [&_h2:not([data-slot=video-status-heading])]:mb-3 [&_h2:not([data-slot=video-status-heading])]:text-[20px] [&_h2:not([data-slot=video-status-heading])]:font-semibold">
      <header className="relative isolate min-h-[300px] pt-0 pb-9 [@media(width<=760px)]:min-h-0">
        {detail.poster_uri && (
          <div className="pointer-events-none absolute -top-[104px] -right-10 -bottom-[70px] left-[35%] -z-1 overflow-hidden opacity-40 [mask-image:linear-gradient(to_right,transparent,black_35%),linear-gradient(to_bottom,black_40%,transparent)] [mask-composite:intersect] [&_img]:object-contain [&_img]:object-[right_top] [&_[data-slot=poster-placeholder]]:hidden [@media(width<=760px)]:-right-6 [@media(width<=760px)]:left-[20%] [@media(width<=760px)]:opacity-25" aria-hidden="true">
            <MediaArtwork uri={detail.poster_uri} title="" eager />
          </div>
        )}
        <div className="max-w-[900px]">
          <Button asChild variant="secondary" className="mb-6">
            <Link to="/library" search={{}}>
              <ArrowLeft size={17} aria-hidden="true" />
              {t("backToLibrary")}
            </Link>
          </Button>
          <h1 className="m-0 max-w-[780px] text-[clamp(28px,3vw,42px)] leading-[1.2] font-[650] tracking-[-0.035em] [overflow-wrap:anywhere]">{detail.media.title}</h1>
          {detail.media.original_title &&
            detail.media.original_title !== detail.media.title && (
              <p className="mt-2 text-[18px] text-foreground">
                {detail.media.original_title}
              </p>
            )}
          <div className="mt-4 flex flex-wrap items-center gap-x-3.5 gap-y-2 text-[14px] text-muted-foreground [&>span+span:not([data-slot=badge])]:before:mr-3.5 [&>span+span:not([data-slot=badge])]:before:content-['·']">
            {detail.media.year !== null && <span>{detail.media.year}</span>}
            <span>{t(`kind_${detail.media.kind}`)}</span>
            {fields.runtime !== null && (
              <span>{t("videoDetailRuntime", { count: fields.runtime })}</span>
            )}
            {fields.genres.length > 0 && <span>{fields.genres.join(" / ")}</span>}
            {fields.rating && (
              <Badge variant="secondary" className="gap-[7px] text-foreground [&_svg]:text-[color:var(--rating)] [&>span]:text-muted-foreground">
                <Star size={17} aria-hidden="true" />
                {number(fields.rating.value)}
                <span>{fields.rating.provider ?? t("localMetadata")}</span>
              </Badge>
            )}
          </div>
          <div className="mt-6 flex flex-wrap items-center gap-3 [&>[data-slot=button]]:min-h-[46px]">
            <NativeOpen files={detail.files} />
            <DetailRecoveryAction detail={detail} />
            <Button asChild variant="outline">
              <Link to="/tools" search={{ media: detail.media.id }}>
                <Wrench size={17} aria-hidden="true" />
                {t("videoTools")}
              </Link>
            </Button>
          </div>
        </div>
      </header>
      <div className="grid grid-cols-[minmax(180px,260px)_minmax(0,1fr)] items-start gap-8 [@media(760px<width<=1350px)]:grid-cols-[minmax(160px,220px)_minmax(0,1fr)] [@media(width<=1350px)]:gap-6 [@media(width<=760px)]:grid-cols-[minmax(0,1fr)]">
        <aside className="[@media(width<=760px)]:max-w-[180px]">
          <div className="poster-image aspect-2/3 overflow-hidden rounded-[14px] border border-input bg-muted">
            <MediaArtwork uri={detail.poster_uri} title={detail.media.title} eager />
          </div>
          <p className="mt-3.5 text-[12px] text-muted-foreground">
            {t("updated")} {date(detail.media.updated_at)}
          </p>
        </aside>
        <div className="min-w-0">
          <div className="mb-8 grid grid-cols-[minmax(0,1fr)_minmax(240px,280px)] items-start gap-6 [@media(width<=1350px)]:grid-cols-[minmax(0,1fr)]">
            <section className="min-w-0">
              {fields.overview && (
                <>
                  <h2 className="mb-3 text-[20px] font-semibold">{t("videoDetailOverview")}</h2>
                  <p
                    className={
                      `whitespace-pre-line leading-[1.8] text-muted-foreground${canExpand && !expanded ? " line-clamp-3" : ""}`
                    }
                  >
                    {fields.overview}
                  </p>
                  {canExpand && (
                    <Button
                      variant="ghost"
                      size="sm"
                      className="mt-1.5 mr-0 mb-0 ml-auto flex text-ring"
                      aria-expanded={expanded}
                      onClick={() => setExpanded(!expanded)}
                    >
                      {t(expanded ? "videoDetailCollapse" : "videoDetailExpand")}
                    </Button>
                  )}
                </>
              )}
              <dl className="mt-6 grid gap-4 [&>div]:grid [&>div]:grid-cols-[90px_minmax(0,1fr)] [&>div]:gap-3 [&>div]:text-[14px] [&_dt]:text-muted-foreground [&_dd]:[overflow-wrap:anywhere]">
                {fields.directors.length > 0 && (
                  <div>
                    <dt>{t("videoDetailDirector")}</dt>
                    <dd>{fields.directors.join(" / ")}</dd>
                  </div>
                )}
                {fields.cast.length > 0 && (
                  <div>
                    <dt>{t("videoDetailCast")}</dt>
                    <dd>{fields.cast.slice(0, 4).join(" / ")}</dd>
                  </div>
                )}
                {fields.premiered && (
                  <div>
                    <dt>{t("videoDetailPremiere")}</dt>
                    <dd>{fields.premiered}</dd>
                  </div>
                )}
                {fields.language && (
                  <div>
                    <dt>{t("language")}</dt>
                    <dd>{fields.language}</dd>
                  </div>
                )}
                {fields.status && (
                  <div>
                    <dt>{t("scraperField_status")}</dt>
                    <dd>{fields.status}</dd>
                  </div>
                )}
                {fields.genres.length > 0 && (
                  <div>
                    <dt>{t("scraperField_genres")}</dt>
                    <dd className="flex flex-wrap gap-1.5">
                      {fields.genres.map((genre) => (
                        <Badge key={genre} variant="secondary">{genre}</Badge>
                      ))}
                    </dd>
                  </div>
                )}
              </dl>
            </section>
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
          </div>
          {sourceDetail}
          <Tabs value={tab} onValueChange={setTab} className="[&>[data-slot=tabs-list]]:flex [&>[data-slot=tabs-list]]:w-full [&>[data-slot=tabs-list]]:justify-start [&>[data-slot=tabs-list]]:gap-2 [&>[data-slot=tabs-list]]:overflow-x-auto [&>[data-slot=tabs-list]]:rounded-none [&>[data-slot=tabs-list]]:border-b [&>[data-slot=tabs-list]]:border-border [&>[data-slot=tabs-list]]:bg-transparent [&>[data-slot=tabs-list]]:p-0 [&>[data-slot=tabs-list]]:pb-2.5 [&_[data-slot=tabs-trigger]]:shrink-0 [&_[data-slot=tabs-trigger]]:min-h-10 [&_[data-slot=tabs-trigger]]:rounded-[10px] [&_[data-slot=tabs-trigger]]:bg-muted [&_[data-slot=tabs-trigger]]:px-[18px] [&_[data-slot=tabs-trigger]]:whitespace-nowrap [&_[data-slot=tabs-trigger][data-state=active]]:bg-primary [&_[data-slot=tabs-trigger][data-state=active]]:text-primary-foreground [&>[data-slot=tabs-content]]:mt-[18px]">
            <TabsList aria-label={t("detailsAction")}>
              <TabsTrigger value="files">{t("videoDetailFiles")}</TabsTrigger>
              {isSeries && (
                <TabsTrigger value="seasons">{t("videoDetailSeasons")}</TabsTrigger>
              )}
              {fields.cast.length > 0 && (
                <TabsTrigger value="cast">{t("videoDetailCast")}</TabsTrigger>
              )}
              <TabsTrigger value="metadata">{t("metadata")}</TabsTrigger>
              {detail.relations.length > 0 && (
                <TabsTrigger value="related">{t("relations")}</TabsTrigger>
              )}
              {detail.artworks.length > 0 && (
                <TabsTrigger value="artwork">{t("artworks")}</TabsTrigger>
              )}
            </TabsList>
            <TabsContent value="files">
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
            </TabsContent>
            {isSeries && (
              <TabsContent value="seasons" aria-busy={source.isFetching}>
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
              </TabsContent>
            )}
            {fields.cast.length > 0 && (
              <TabsContent value="cast">
                <ul className="m-0 grid list-none grid-cols-[repeat(auto-fit,minmax(140px,1fr))] gap-3 p-0">
                  {fields.cast.map((name) => (
                    <Card asChild key={name} className="p-4 sm:p-4">
                      <li>{name}</li>
                    </Card>
                  ))}
                </ul>
              </TabsContent>
            )}
            <TabsContent value="metadata">
              {detail.metadata_values.length ? (
                <dl className="[&>div]:border-b [&>div]:border-border [&>div]:py-4 [&_dt]:flex [&_dt]:flex-wrap [&_dt]:items-center [&_dt]:gap-2 [&_dt]:text-[14px] [&_dt]:font-medium [&_dd]:mt-2 [&_dd]:[overflow-wrap:anywhere]">
                  {detail.metadata_values.map((field) => (
                    <div key={field.id}>
                      <dt>
                        {t(`scraperField_${field.field_key}`, {
                          defaultValue: field.field_key,
                        })}
                        {field.locale && <Badge variant="secondary">{field.locale}</Badge>}
                        {field.is_locked && (
                          <Badge variant="secondary">
                            <LockKeyhole size={12} aria-hidden="true" />{t("locked")}
                          </Badge>
                        )}
                        {field.is_user_modified && (
                          <Badge variant="secondary">{t("userEdited")}</Badge>
                        )}
                      </dt>
                      <dd className="text-[14px]">
                        {typeof field.value === "string"
                          ? field.value
                          : JSON.stringify(field.value)}
                      </dd>
                      <dd className="text-[12px] text-muted-foreground">
                        {t("source")}: {field.provider_id ?? t("localMetadata")}
                      </dd>
                    </div>
                  ))}
                </dl>
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
            </TabsContent>
            {detail.relations.length > 0 && (
              <TabsContent value="related">
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
              </TabsContent>
            )}
            {detail.artworks.length > 0 && (
              <TabsContent value="artwork">
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
              </TabsContent>
            )}
          </Tabs>
        </div>
      </div>
    </article>
  );
}
