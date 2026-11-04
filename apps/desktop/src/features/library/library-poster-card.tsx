import { Link } from "@tanstack/react-router";
import { useId } from "react";
import { useTranslation } from "react-i18next";
import type { MediaCard } from "../../api/library";
import { Badge } from "../../components/ui/badge";
import { Button } from "../../components/ui/button";
import { Card } from "../../components/ui/card";
import { MediaArtwork } from "./media-artwork";
import "./browse-translations";
import "./library-status-translations";

const metadataStates = [
  "metadata_needs_review",
  "metadata_failed",
  "metadata_unconfigured",
  "metadata_pending",
];

/** A poster selects a title. Opening an external app remains an explicit detail action. */
export function LibraryPosterCard({
  card,
  adult,
}: {
  card: MediaCard;
  adult: boolean;
}) {
  const { t } = useTranslation();
  const metadataId = useId();
  const statusId = useId();
  const state = metadataStates.find((value) => card.health.includes(value));
  return (
    <Card asChild className="border-0 bg-transparent p-0 sm:p-0">
      <article className="poster-card library-poster-card">
        <Button
          asChild
          variant="ghost"
          className="library-poster-link h-auto w-full items-stretch justify-start gap-0 bg-transparent p-0 text-left text-foreground hover:bg-transparent"
        >
          <Link
            to="/media/$mediaId"
            params={{ mediaId: card.media.id }}
            search={adult ? { kind: "adult_video" } : {}}
            aria-label={t("libraryPosterOpen", { title: card.media.title })}
            aria-describedby={state ? `${metadataId} ${statusId}` : metadataId}
          >
            <div className="poster-image">
              <MediaArtwork uri={card.poster_uri} title={card.media.title} />
              {state && (
                <span className="library-poster-status" id={statusId}>
                  <Badge variant="secondary">{t(`health_${state}`)}</Badge>
                </span>
              )}
            </div>
            <span className="library-poster-caption">
              <span className="library-poster-title" title={card.media.title}>
                {card.media.title}
              </span>
              <span className="library-poster-meta" id={metadataId}>
                {[card.media.year, t(`kind_${card.media.kind}`)]
                  .filter(Boolean)
                  .join(" · ")}
              </span>
            </span>
          </Link>
        </Button>
      </article>
    </Card>
  );
}
