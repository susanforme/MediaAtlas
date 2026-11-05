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
    <Card asChild className="min-w-0 overflow-visible rounded-[10px] border-0 bg-transparent p-0 sm:p-0">
      <article className="poster-card library-poster-card">
        <Button
          asChild
          variant="ghost"
          className="group flex h-auto w-full min-w-0 flex-col items-stretch justify-start gap-0 rounded-[10px] bg-transparent p-0 text-left whitespace-normal text-foreground hover:bg-transparent [&:hover]:text-foreground"
        >
          <Link
            to="/media/$mediaId"
            params={{ mediaId: card.media.id }}
            search={adult ? { kind: "adult_video" } : {}}
            aria-label={t("libraryPosterOpen", { title: card.media.title })}
            aria-describedby={state ? `${metadataId} ${statusId}` : metadataId}
          >
            <div data-slot="poster-image" className="poster-image relative aspect-2/3 w-full overflow-hidden rounded-[10px] bg-muted transition-[filter] duration-150 ease-[ease] group-[:hover]:brightness-[1.08] motion-reduce:transition-none">
              <MediaArtwork uri={card.poster_uri} title={card.media.title} />
              {state && (
                <span className="absolute inset-x-2 bottom-2 flex" id={statusId}>
                  <Badge variant="secondary" className="max-w-full overflow-hidden bg-background/92 text-[10px] leading-[var(--text-xs--line-height)] text-ellipsis whitespace-nowrap text-foreground">{t(`health_${state}`)}</Badge>
                </span>
              )}
            </div>
            <span className="grid w-full min-w-0 gap-1 pt-[9px] text-left">
              <span className="block truncate text-sm leading-5 font-medium" title={card.media.title}>
                {card.media.title}
              </span>
              <span className="truncate text-xs leading-[18px] font-normal text-muted-foreground" id={metadataId}>
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
