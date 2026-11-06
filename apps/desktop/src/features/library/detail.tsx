import { Card } from "../../components/ui/card";
import { MediaArtwork } from "./media-artwork";
import { useQuery } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { LockKeyhole } from "lucide-react";
import { lazy, type ReactNode, Suspense } from "react";
import { useTranslation } from "react-i18next";
import { mediaDetailQuery } from "./queries";
import { Badge } from "../../components/ui/badge";
import { Button } from "../../components/ui/button";
import { usePrivacy } from "./context";
import { DetailRecoveryAction } from "./detail-recovery-action";
import { NativeOpen } from "./native-open";
import { useLibraryActivity } from "./use-library-activity";
import { VideoDetail } from "./video-detail";
import "../scraper/translations";
import "./library-status-translations";
import {
  Empty,
  ErrorNotice,
  HealthBadges,
  Loading,
  PageHeading,
  useFormat,
} from "./shared";

const LibraryVideoSourceDetail = lazy(() =>
  import("../scraper/source-detail").then((module) => ({
    default: module.LibraryVideoSourceDetail,
  })),
);

export function MediaDetailPage({
  mediaId,
  adult = false,
  sourceDetail,
}: {
  mediaId: string;
  adult?: boolean;
  sourceDetail?: ReactNode;
}) {
  useLibraryActivity(undefined, !adult);
  const { t } = useTranslation();
  const { number, date } = useFormat();
  const { settings, includeAdult } = usePrivacy();
  const allowed = !adult || (settings.entry_visible && includeAdult);
  const detail = useQuery({
    ...mediaDetailQuery(mediaId, adult),
    enabled: allowed,
  });
  if (!allowed)
    return settings.entry_visible ? (
      <Loading />
    ) : (
      <Empty title={t("noResults")} />
    );
  if (detail.isPending) return <Loading />;
  if (detail.isError)
    return (
      <ErrorNotice error={detail.error} retry={() => void detail.refetch()} />
    );
  const data = detail.data;
  if (adult !== (data.media.kind === "adult_video"))
    return <Empty title={t("noResults")} />;
  if (!adult && ["movie", "tv_series"].includes(data.media.kind))
    return (
      <VideoDetail
        key={data.media.id}
        detail={data}
        sourceDetail={sourceDetail}
      />
    );
  return (
    <>
      <Button asChild variant="ghost">
        <Link
          to="/library"
          search={adult ? { kind: "adult_video" } : {}}
          className="mb-5 inline-block text-sm text-ring"
        >
          ← {t("backToLibrary")}
        </Link>
      </Button>
      <PageHeading
        title={data.media.title}
        description={`${t(`kind_${data.media.kind}`)} · ${data.media.year ?? t("unknownYear")}`}
      >
        <NativeOpen files={data.files} adult={adult} />
        <DetailRecoveryAction detail={data} />
        <Button asChild variant="ghost">
          <Link
            to="/tools"
            search={
              adult
                ? { kind: "adult_video", media: data.media.id }
                : { media: data.media.id }
            }
          >
            {t("videoTools")}
          </Link>
        </Button>
      </PageHeading>
      <div className="grid grid-cols-[220px_minmax(0,1fr)] gap-7 [@media(width<=760px)]:grid-cols-[minmax(0,1fr)] [@media(width<=760px)]:[&>div:first-child]:max-w-[220px]">
        <div>
          <div className="poster-image aspect-2/3 overflow-hidden rounded-[10px] border border-border bg-muted">
            <MediaArtwork
              uri={adult && !settings.covers_visible ? null : data.poster_uri}
              title={data.media.title}
              eager
            />
          </div>
          <div className="mt-4">
            <HealthBadges
              health={data.health.filter(
                (health) =>
                  ![
                    "metadata_pending",
                    "missing_nfo",
                    "nfo_output_failed",
                  ].includes(health),
              )}
            />
          </div>
          <p className="mt-4 text-xs text-muted-foreground">
            {t("updated")} {date(data.media.updated_at)}
          </p>
        </div>
        <div className="space-y-6">
          {sourceDetail}
          {["movie", "tv_series", "anime"].includes(data.media.kind) && (
            <Suspense fallback={<Loading />}>
              <LibraryVideoSourceDetail mediaId={mediaId} />
            </Suspense>
          )}
          {(data.health.includes("missing_nfo") ||
            data.health.includes("nfo_output_failed")) && (
            <Card asChild>
              <section
                className="space-y-3"
                aria-label={t("libraryNfoOutput")}>
                <div className="flex flex-wrap items-center gap-3">
                  <h2 className="mb-4 font-semibold">{t("libraryNfoOutput")}</h2>
                  <Badge
                    variant={
                      data.health.includes("nfo_output_failed")
                        ? "destructive"
                        : "secondary"
                    }
                  >
                    {t(
                      data.health.includes("nfo_output_failed")
                        ? "health_nfo_output_failed"
                        : "health_missing_nfo",
                    )}
                  </Badge>
                </div>
                <p className="text-sm text-muted-foreground">
                  {t(
                    data.health.includes("nfo_output_failed")
                      ? "libraryNfoFailedHint"
                      : "libraryNfoPendingHint",
                  )}
                </p>
                <Button asChild variant="outline" size="sm">
                  <Link to="/roots" search={adult ? { kind: "adult_video" } : {}}>
                    {t("libraryRefreshManage")}
                  </Link>
                </Button>
              </section>
            </Card>
          )}
          <Card asChild>
            <section>
              <h2 className="mb-4 font-semibold">{t("files")}</h2>
              {data.files.length ? (
                <ul className="divide-y divide-border">
                  {data.files.map((file) => (
                    <li className="py-3" key={file.id}>
                      <p className="break-all text-sm font-medium">
                        {file.relative_path}
                      </p>
                      <p className="mt-1 text-xs text-muted-foreground">
                        {number(file.size_bytes)} B{" "}
                        {file.part !== null &&
                          ` · ${t("part")} ${number(file.part)}`}
                      </p>
                    </li>
                  ))}
                </ul>
              ) : (
                <p>{t("noFiles")}</p>
              )}
            </section>
          </Card>
          <Card asChild>
            <section>
              <h2 className="mb-4 font-semibold">{t("metadata")}</h2>
              {data.metadata_values.length ? (
                <dl className="divide-y divide-border">
                  {data.metadata_values.map((field) => (
                    <div key={field.id} className="py-3">
                      <dt className="flex flex-wrap items-center gap-2 text-sm font-medium">
                        {field.field_key}
                        {field.locale && (
                          <Badge variant="secondary">{field.locale}</Badge>
                        )}
                        {field.is_locked && (
                          <Badge variant="secondary">
                            <LockKeyhole size={11} aria-hidden="true" />{" "}
                            {t("locked")}
                          </Badge>
                        )}
                        {field.is_user_modified && (
                          <Badge variant="secondary">{t("userEdited")}</Badge>
                        )}
                      </dt>
                      <dd className="mt-2 break-words text-sm">
                        {typeof field.value === "string"
                          ? field.value
                          : JSON.stringify(field.value)}
                      </dd>
                      <dd className="mt-1 text-xs text-muted-foreground">
                        {t("source")}: {field.provider_id ?? t("localMetadata")}
                      </dd>
                    </div>
                  ))}
                </dl>
              ) : (
                <p className="text-sm text-muted-foreground">{t("noMetadata")}</p>
              )}
            </section>
          </Card>
          <Card asChild>
            <section>
              <h2 className="mb-4 font-semibold">{t("externalIds")}</h2>
              {data.external_ids.length ? (
                <ul>
                  {data.external_ids.map((id) => (
                    <li key={id.id} className="break-all py-1 text-sm">
                      {id.provider_id} / {id.namespace}: {id.external_id}
                    </li>
                  ))}
                </ul>
              ) : (
                <p className="text-sm text-muted-foreground">{t("none")}</p>
              )}
            </section>
          </Card>
          <Card asChild>
            <section>
              <h2 className="mb-4 font-semibold">{t("artworks")}</h2>
              {data.artworks.length ? (
                <ul>
                  {data.artworks.map((art) => (
                    <li key={art.id} className="break-all py-1 text-sm">
                      {art.kind} · {art.cache_path ?? t("none")}
                    </li>
                  ))}
                </ul>
              ) : (
                <p className="text-sm text-muted-foreground">{t("noPoster")}</p>
              )}
            </section>
          </Card>
          {data.relations.length > 0 && (
            <Card asChild>
              <section>
                <h2 className="mb-4 font-semibold">{t("relations")}</h2>
                <ul>
                  {data.relations.map((relation) => (
                    <li key={relation.id} className="py-1">
                      <Button asChild variant="ghost">
                        <Link
                          to="/media/$mediaId"
                          params={{ mediaId: relation.id }}
                          search={adult ? { kind: "adult_video" } : {}}
                          className="text-ring"
                        >
                          {relation.title}
                        </Link>
                      </Button>
                      <span className="ml-2 text-xs text-muted-foreground">
                        {t(`kind_${relation.kind}`)}
                      </span>
                    </li>
                  ))}
                </ul>
              </section>
            </Card>
          )}
        </div>
      </div>
    </>
  );
}
