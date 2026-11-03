import { useTranslation } from "react-i18next";
import { Button } from "../../components/ui/button";

export function ScraperQueueNavigation({
  mediaId,
  queue,
  busy,
  onNavigate,
}: {
  mediaId: string;
  queue: string[];
  busy: boolean;
  onNavigate?: (mediaId: string) => void;
}) {
  const { t } = useTranslation();
  const index = queue.indexOf(mediaId);
  const previous = queue[index - 1];
  const next = queue[index + 1];
  if (index === -1) return null;
  function navigate(id: string | undefined) {
    if (id && !busy) onNavigate?.(id);
  }
  return (
    <div className="mb-4 flex flex-wrap items-center justify-between gap-3 border-b border-border pb-4">
      <p className="text-sm font-medium" aria-live="polite">
        {t("scraperQueuePosition", { index: index + 1, count: queue.length })}
      </p>
      <div className="flex flex-wrap gap-2">
        <Button
          variant="outline"
          size="sm"
          disabled={busy || !previous || !onNavigate}
          onClick={() => navigate(previous)}
        >
          {t("scraperQueuePrevious")}
        </Button>
        <Button
          variant="outline"
          size="sm"
          disabled={busy || !next || !onNavigate}
          onClick={() => navigate(next)}
        >
          {t("scraperQueueNext")}
        </Button>
        <Button
          variant="ghost"
          size="sm"
          disabled={busy || !next || !onNavigate}
          onClick={() => navigate(next)}
        >
          {t("scraperQueueSkip")}
        </Button>
      </div>
    </div>
  );
}
