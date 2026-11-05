/** biome-ignore-all lint/suspicious/noArrayIndexKey: Immutable provider snapshots can lack unique season/episode identifiers. */
import { Card } from "../../components/ui/card";
import { useQuery } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { scraperApi, type VideoMetadata } from "../../api/scraper";
import { Details, DetailsSummary } from "../../components/ui/details";
import { ErrorNotice, Loading, useFormat } from "../library/shared";
import "./translations";

/** A confirmed provider snapshot supplements the separately protected library fields. */
export function VideoSourceDetails({ metadata }: { metadata: VideoMetadata }) {
  const { t } = useTranslation();
  const { number } = useFormat();
  return (
    <>
      <dl className="mt-4 grid grid-cols-[minmax(8rem,auto)_1fr] gap-x-4 gap-y-[0.6rem] [&_dt]:text-muted-foreground [&_dd]:min-w-0 [@media(max-width:640px)]:grid-cols-[1fr] [@media(max-width:640px)]:gap-[0.3rem]">
        <dt>{t("scraperIdentity")}</dt>
        <dd>
          {metadata.data.identity.provider_id} /{" "}
          {metadata.data.identity.remote_id}
        </dd>
        <dt>{t("scraperLanguage")}</dt>
        <dd>{metadata.data.identity.language}</dd>
        <dt>{t("scraperSource")}</dt>
        <dd className="break-all">{metadata.data.identity.source_url}</dd>
        <dt>{t("externalIds")}</dt>
        <dd>
          {metadata.data.identity.external_ids
            .map((id) => `${id.namespace}: ${id.value}`)
            .join(" · ") || t("none")}
        </dd>
        {metadata.kind === "movie" && (
          <>
            <dt>{t("scraperRuntime")}</dt>
            <dd>
              {metadata.data.runtime_minutes === null
                ? t("scraperUnknown")
                : number(metadata.data.runtime_minutes)}
            </dd>
          </>
        )}
        {metadata.kind === "anime" && (
          <>
            <dt>{t("scraperAliases")}</dt>
            <dd>{metadata.data.aliases.join(" · ") || t("none")}</dd>
            <dt>{t("scraperFormat")}</dt>
            <dd>
              {metadata.data.format
                ? t(`scraperFormat_${metadata.data.format}`, {
                    defaultValue: metadata.data.format,
                  })
                : t("scraperUnknown")}
            </dd>
            <dt>{t("season")}</dt>
            <dd>
              {metadata.data.season
                ? t(`scraperSeason_${metadata.data.season}`, {
                    defaultValue: metadata.data.season,
                  })
                : t("scraperUnknown")}
            </dd>
            <dt>{t("scraperEpisodes")}</dt>
            <dd>
              {metadata.data.episodes === null
                ? t("scraperUnknown")
                : number(metadata.data.episodes)}
            </dd>
          </>
        )}
      </dl>
      {metadata.kind === "tv" && (
        <div className="mt-5">
          <h3 className="font-medium">
            {t("scraperOrder")}: {t(`scraperOrder_${metadata.data.order}`)}
          </h3>
          {metadata.data.seasons.map((season, index) => (
            <Details className="mt-3" key={index}>
              <DetailsSummary>
                {t("season")} {number(season.season_number)} · {season.title}
              </DetailsSummary>
              <ol className="mt-2 space-y-2 pl-5">
                {season.episodes.map((episode, i) => (
                  <li key={i}>
                    {t("episode")} {number(episode.episode_number)}:{" "}
                    {episode.title}
                    {episode.overview && (
                      <p className="text-sm text-muted-foreground">
                        {episode.overview}
                      </p>
                    )}
                  </li>
                ))}
              </ol>
            </Details>
          ))}
        </div>
      )}
    </>
  );
}
export function LibraryVideoSourceDetail({ mediaId }: { mediaId: string }) {
  const { t } = useTranslation();
  const detail = useQuery({
    queryKey: ["video-source-detail", mediaId],
    queryFn: () => scraperApi.mediaDetail(mediaId),
  });
  return (
    <Card asChild>
      <section aria-labelledby="video-source-title">
        <h2 id="video-source-title" className="mb-4 font-semibold">
          {t("scraperLibrarySourceTitle")}
        </h2>
        {detail.isPending ? (
          <Loading />
        ) : detail.error ? (
          <ErrorNotice error={detail.error} retry={() => void detail.refetch()} />
        ) : detail.data ? (
          <VideoSourceDetails metadata={detail.data} />
        ) : (
          <p className="mt-4 text-sm text-muted-foreground">
            {t("scraperLibrarySourceEmpty")}
          </p>
        )}
      </section>
    </Card>
  );
}
