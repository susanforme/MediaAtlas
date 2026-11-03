import { Link } from "@tanstack/react-router";
import { ArrowRight, ChevronRight, Film, Info, Plus } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "../../components/ui/button";
import { MediaArtwork } from "../library/media-artwork";
import { NativeOpen } from "../library/native-open";
import { ErrorNotice, Loading, useFormat } from "../library/shared";
import { featuredBackdrop, featuredDescription } from "./media-model";
import { useHomeLibrary } from "./use-home-library";
import "./translations";
import "../library/library.css";
import "./home.css";

export function DashboardPage() {
  const { t } = useTranslation();
  const { number } = useFormat();
  const { summary, media, cards, featured, detail } = useHomeLibrary();
  const [failedBackdrop, setFailedBackdrop] = useState<string>();
  const backdrop = featuredBackdrop(detail.data);
  const description = featuredDescription(detail.data);
  return (
    <div className="home-page">
      <section className="home-hero" aria-label={t("homeFeatured")}>
        {backdrop && failedBackdrop !== backdrop && (
          <img
            className="home-hero-art"
            src={backdrop}
            alt=""
            onError={() => setFailedBackdrop(backdrop)}
          />
        )}
        {!backdrop && featured && (
          <div className="home-hero-poster" aria-hidden="true">
            <MediaArtwork uri={featured.poster_uri} title="" eager />
          </div>
        )}
        <div className="home-hero-shade" />
        <div className="home-hero-content">
          <p className="home-eyebrow">
            <Film size={17} aria-hidden="true" /> MediaAtlas
          </p>
          {media.isPending ? (
            <Loading />
          ) : media.isError ? (
            <>
              <h1>{t("dashboard")}</h1>
              <ErrorNotice
                error={media.error}
                retry={() => void media.refetch()}
              />
            </>
          ) : featured ? (
            <>
              <h1>{featured.media.title}</h1>
              <p className="home-meta">
                {featured.media.year && <span>{featured.media.year}</span>}
                <span>{t(`kind_${featured.media.kind}`)}</span>
              </p>
              {description && <p className="home-synopsis">{description}</p>}
              <div className="home-actions">
                <NativeOpen
                  mediaId={featured.media.id}
                  label={featured.media.title}
                />
                <Button variant="secondary" asChild>
                  <Link
                    to="/media/$mediaId"
                    params={{ mediaId: featured.media.id }}
                  >
                    <Info size={18} aria-hidden="true" />
                    {t("homeOpenDetails")}
                  </Link>
                </Button>
              </div>
            </>
          ) : (
            <>
              <h1>
                {t(
                  summary.data?.total_roots ? "homeReadyToScan" : "homeWelcome",
                )}
              </h1>
              <p className="home-synopsis">
                {t(
                  summary.data?.total_roots
                    ? "homeScanBeforeBrowse"
                    : "homeNewLibrary",
                )}
              </p>
              <div className="home-actions">
                {summary.data?.total_roots ? (
                  <>
                    <Button asChild>
                      <Link
                        to={
                          summary.data.pending_tasks > 0 ? "/tasks" : "/scanner"
                        }
                      >
                        {t(
                          summary.data.pending_tasks > 0
                            ? "homeReviewTasks"
                            : "startScan",
                        )}
                      </Link>
                    </Button>
                    {summary.data.pending_tasks > 0 && (
                      <Button asChild variant="outline">
                        <Link to="/scanner">{t("scanner")}</Link>
                      </Button>
                    )}
                  </>
                ) : (
                  <Button asChild>
                    <Link to="/roots">
                      <Plus size={18} aria-hidden="true" />
                      {t("addRoot")}
                    </Link>
                  </Button>
                )}
              </div>
            </>
          )}
        </div>
      </section>
      <div className="home-shelves">
        {cards.length > 0 && (
          <>
            <section aria-labelledby="home-explore">
              <div className="home-section-heading">
                <h2 id="home-explore">{t("homeExplore")}</h2>
                <Button variant="ghost" size="sm" asChild>
                  <Link to="/library">
                    {t("browseLibrary")}
                    <ChevronRight size={18} aria-hidden="true" />
                  </Link>
                </Button>
              </div>
              <div className="home-landscape-row">
                {cards.slice(0, 4).map((card) => (
                  <article className="home-landscape-card" key={card.media.id}>
                    <div className="home-landscape-art">
                      <MediaArtwork
                        uri={card.poster_uri}
                        title={card.media.title}
                      />
                    </div>
                    <div className="home-landscape-info">
                      <h3>{card.media.title}</h3>
                      <p>
                        {card.media.year ?? t("unknownYear")} ·{" "}
                        {t(`kind_${card.media.kind}`)}
                      </p>
                      <Button variant="link" size="sm" asChild>
                        <Link
                          to="/media/$mediaId"
                          params={{ mediaId: card.media.id }}
                        >
                          {t("detailsAction")}
                          <ArrowRight size={14} aria-hidden="true" />
                        </Link>
                      </Button>
                    </div>
                  </article>
                ))}
              </div>
            </section>
            <section aria-labelledby="home-recent">
              <div className="home-section-heading">
                <h2 id="home-recent">{t("homeRecent")}</h2>
                <Button variant="ghost" size="sm" asChild>
                  <Link to="/library" search={{ sort: "added_desc" }}>
                    {t("browseLibrary")}
                    <ChevronRight size={18} aria-hidden="true" />
                  </Link>
                </Button>
              </div>
              <div className="home-poster-row">
                {cards.map((card) => (
                  <article className="home-poster-card" key={card.media.id}>
                    <NativeOpen
                      mediaId={card.media.id}
                      label={card.media.title}
                      className="home-poster-open"
                    >
                      <div className="poster-image">
                        <MediaArtwork
                          uri={card.poster_uri}
                          title={card.media.title}
                        />
                      </div>
                      <span className="home-poster-title">
                        {card.media.title}
                      </span>
                    </NativeOpen>
                    <p className="home-poster-meta">
                      {card.media.year ?? t("unknownYear")} ·{" "}
                      {t(`kind_${card.media.kind}`)}
                    </p>
                  </article>
                ))}
              </div>
            </section>
          </>
        )}
        {summary.isSuccess && (
          <section className="home-library-status" aria-label={t("homeHealth")}>
            <span>
              {t("totalMedia")}{" "}
              <strong>{number(summary.data.total_media)}</strong>
            </span>
            <span>
              {t("totalRoots")}{" "}
              <strong>{number(summary.data.total_roots)}</strong>
            </span>
            <span>
              {t("pendingTasks")}{" "}
              <strong>{number(summary.data.pending_tasks)}</strong>
            </span>
            {summary.data.failed_tasks > 0 && (
              <Button variant="link" asChild>
                <Link to="/tasks">
                  {t("failedTasks")} {number(summary.data.failed_tasks)}
                </Link>
              </Button>
            )}
          </section>
        )}
      </div>
    </div>
  );
}
