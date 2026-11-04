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
import "./video-detail.css";

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
    <div className="video-seasons">
      <div
        className="video-season-list"
        role="group"
        aria-label={t("videoDetailSeasons")}
      >
        {seasons.map((item) => (
          <Button
            key={`${item.season_number}:${item.remote_id}`}
            variant="ghost"
            className="video-season-card"
            aria-pressed={item === season}
            onClick={() => setSelected(item.season_number)}
          >
            <Film size={30} aria-hidden="true" />
            <span>
              {t("videoDetailSeason", {
                number: number(item.season_number),
              })}
            </span>
            <span className="video-season-count">
              {t("videoDetailSourceEpisodes", { count: item.episodes.length })}
            </span>
          </Button>
        ))}
      </div>
      <h3 className="video-episode-heading">
        {season.title ||
          t("videoDetailSeason", { number: number(season.season_number) })}
      </h3>
      <ol className="video-episodes">
        {season.episodes.map((episode) => (
          <Card asChild key={`${episode.episode_number}:${episode.remote_id}`}>
            <li>
              <span className="video-episode-number">
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
    <article className="video-detail">
      <header className="video-detail-hero">
        {detail.poster_uri && (
          <div className="video-detail-hero-art" aria-hidden="true">
            <MediaArtwork uri={detail.poster_uri} title="" eager />
          </div>
        )}
        <div className="video-detail-heading">
          <Button asChild variant="secondary" className="video-detail-back">
            <Link to="/library" search={{}}>
              <ArrowLeft size={17} aria-hidden="true" />
              {t("backToLibrary")}
            </Link>
          </Button>
          <h1>{detail.media.title}</h1>
          {detail.media.original_title &&
            detail.media.original_title !== detail.media.title && (
              <p className="video-detail-original">
                {detail.media.original_title}
              </p>
            )}
          <div className="video-detail-meta">
            {detail.media.year !== null && <span>{detail.media.year}</span>}
            <span>{t(`kind_${detail.media.kind}`)}</span>
            {fields.runtime !== null && (
              <span>{t("videoDetailRuntime", { count: fields.runtime })}</span>
            )}
            {fields.genres.length > 0 && <span>{fields.genres.join(" / ")}</span>}
            {fields.rating && (
              <Badge variant="secondary" className="video-detail-rating">
                <Star size={17} aria-hidden="true" />
                {number(fields.rating.value)}
                <span>{fields.rating.provider ?? t("localMetadata")}</span>
              </Badge>
            )}
          </div>
          <div className="video-detail-actions">
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
      <div className="video-detail-layout">
        <aside className="video-detail-poster">
          <div className="poster-image">
            <MediaArtwork uri={detail.poster_uri} title={detail.media.title} eager />
          </div>
          <p>
            {t("updated")} {date(detail.media.updated_at)}
          </p>
        </aside>
        <div className="video-detail-body">
          <div className="video-detail-overview-grid">
            <section className="video-detail-overview">
              {fields.overview && (
                <>
                  <h2>{t("videoDetailOverview")}</h2>
                  <p
                    className={
                      canExpand && !expanded ? "video-detail-summary" : undefined
                    }
                  >
                    {fields.overview}
                  </p>
                  {canExpand && (
                    <Button
                      variant="ghost"
                      size="sm"
                      aria-expanded={expanded}
                      onClick={() => setExpanded(!expanded)}
                    >
                      {t(expanded ? "videoDetailCollapse" : "videoDetailExpand")}
                    </Button>
                  )}
                </>
              )}
              <dl className="video-detail-facts">
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
                    <dd className="video-detail-genres">
                      {fields.genres.map((genre) => (
                        <Badge key={genre} variant="secondary">{genre}</Badge>
                      ))}
                    </dd>
                  </div>
                )}
              </dl>
            </section>
            <Card asChild>
              <aside
                className="video-detail-status"
                aria-label={t("videoDetailStatus")}
              >
                <h2>{t("videoDetailStatus")}</h2>
                <HealthBadges health={visibleHealth} />
                <dl>
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
                    className="video-detail-nfo"
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
          <Tabs value={tab} onValueChange={setTab} className="video-detail-tabs">
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
                <ul className="video-detail-files">
                  {detail.files.map((file) => (
                    <Card asChild key={file.id}>
                      <li>
                        <FileVideo size={25} aria-hidden="true" />
                        <div className="video-detail-file-name">
                          <h3>{file.relative_path.split(/[\\/]/).at(-1)}</h3>
                          <p>{file.relative_path}</p>
                        </div>
                        <span className="video-detail-file-size">
                          {number(file.size_bytes)} B
                          {file.part !== null && ` · ${t("part")} ${number(file.part)}`}
                        </span>
                        {playable.some((item) => item.id === file.id) && (
                          <NativeOpen
                            files={[file]}
                            label={file.relative_path}
                            className="video-detail-file-open"
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
                <ul className="video-detail-cast">
                  {fields.cast.map((name) => (
                    <Card asChild key={name}>
                      <li>{name}</li>
                    </Card>
                  ))}
                </ul>
              </TabsContent>
            )}
            <TabsContent value="metadata">
              {detail.metadata_values.length ? (
                <dl className="video-detail-metadata">
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
                      <dd>
                        {typeof field.value === "string"
                          ? field.value
                          : JSON.stringify(field.value)}
                      </dd>
                      <dd className="video-detail-field-source">
                        {t("source")}: {field.provider_id ?? t("localMetadata")}
                      </dd>
                    </div>
                  ))}
                </dl>
              ) : (
                <p className="text-muted-foreground">{t("noMetadata")}</p>
              )}
              <section className="video-detail-source">
                <h3>{t("scraperLibrarySourceTitle")}</h3>
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
                <ul className="video-detail-related">
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
                <ul className="video-detail-artwork">
                  {detail.artworks.map((art) => (
                    <Card asChild key={art.id}>
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
