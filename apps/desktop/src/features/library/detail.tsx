import { useQuery } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { ImageOff, LockKeyhole } from "lucide-react";
import { lazy, Suspense } from "react";
import { useTranslation } from "react-i18next";
import { libraryApi } from "../../api/library";
import { NativeOpen } from "./native-open";
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

export function MediaDetailPage({ mediaId }: { mediaId: string }) {
  const { t } = useTranslation();
  const { number, date } = useFormat();
  const detail = useQuery({
    queryKey: ["detail", mediaId, false],
    queryFn: () => libraryApi.detail(mediaId, false),
  });
  if (detail.isPending) return <Loading />;
  if (detail.isError)
    return (
      <ErrorNotice error={detail.error} retry={() => void detail.refetch()} />
    );
  const data = detail.data;
  if (data.media.kind === "adult_video")
    return <Empty title={t("privacyHidden")} />;
  return (
    <>
      <Link to="/library" className="mb-5 inline-block text-sm text-primary">
        ← {t("backToLibrary")}
      </Link>
      <PageHeading
        title={data.media.title}
        description={`${t(`kind_${data.media.kind}`)} · ${data.media.year ?? t("unknownYear")}`}
      >
        <NativeOpen files={data.files} />
        <Link to="/tools" className="action-link">
          {t("videoTools")}
        </Link>
      </PageHeading>
      <div className="detail-layout">
        <div>
          <div className="poster-image rounded-2xl border border-border">
            {data.poster_uri?.startsWith("data:image/") ? (
              <img alt={data.media.title} src={data.poster_uri} />
            ) : (
              <div className="poster-placeholder">
                <ImageOff size={30} aria-hidden="true" />
                <span>{t("noPoster")}</span>
              </div>
            )}
          </div>
          <div className="mt-4">
            <HealthBadges health={data.health} />
          </div>
          <p className="mt-4 text-xs text-muted-foreground">
            {t("updated")} {date(data.media.updated_at)}
          </p>
        </div>
        <div className="space-y-6">
          {["movie", "tv_series", "anime"].includes(data.media.kind) && (
            <Suspense fallback={<Loading />}>
              <LibraryVideoSourceDetail mediaId={mediaId} />
            </Suspense>
          )}
          <section className="panel">
            <h2 className="section-title">{t("files")}</h2>
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
          <section className="panel">
            <h2 className="section-title">{t("metadata")}</h2>
            {data.metadata_values.length ? (
              <dl className="divide-y divide-border">
                {data.metadata_values.map((field) => (
                  <div key={field.id} className="py-3">
                    <dt className="flex flex-wrap items-center gap-2 text-sm font-medium">
                      {field.field_key}
                      {field.locale && (
                        <span className="badge">{field.locale}</span>
                      )}
                      {field.is_locked && (
                        <span className="badge">
                          <LockKeyhole size={11} aria-hidden="true" />{" "}
                          {t("locked")}
                        </span>
                      )}
                      {field.is_user_modified && (
                        <span className="badge">{t("userEdited")}</span>
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
          <section className="panel">
            <h2 className="section-title">{t("externalIds")}</h2>
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
          <section className="panel">
            <h2 className="section-title">{t("artworks")}</h2>
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
          {data.relations.length > 0 && (
            <section className="panel">
              <h2 className="section-title">{t("relations")}</h2>
              <ul>
                {data.relations.map((relation) => (
                  <li key={relation.id} className="py-1">
                    <Link
                      to="/media/$mediaId"
                      params={{ mediaId: relation.id }}
                      className="text-primary"
                    >
                      {relation.title}
                    </Link>
                    <span className="ml-2 text-xs text-muted-foreground">
                      {t(`kind_${relation.kind}`)}
                    </span>
                  </li>
                ))}
              </ul>
            </section>
          )}
        </div>
      </div>
    </>
  );
}
