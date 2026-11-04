import { Link } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";
import { Button } from "../../components/ui/button";
import { ErrorNotice } from "../library/shared";
import { validateToolsSearch } from "../tools/search";
import type { ReviewSession } from "./use-review-queue";
import "./review-queue-translations";

export function PersistentQueueNavigation({
  review,
  busy,
  onNavigate,
}: {
  review: ReviewSession;
  busy: boolean;
  onNavigate?: (mediaId: string) => void;
}) {
  const { t } = useTranslation();
  const queue = review.query.data;
  if (!queue || !review.selected) return null;
  const selected = review.selected;
  const counts = Object.fromEntries(
    ["pending", "confirmed", "deferred", "failed"].map((state) => [
      state,
      queue.items.filter((item) => item.state === state).length,
    ]),
  );
  async function choose(mediaId: string) {
    if (busy) return;
    const item = queue?.items.find((entry) => entry.media_id === mediaId);
    if (
      item?.state === "pending" &&
      !item.needs_recheck &&
      queue?.current_media_id !== mediaId
    ) {
      try {
        await review.transition("select", mediaId);
      } catch {
        return;
      }
    }
    if (review.isActive()) onNavigate?.(mediaId);
  }
  async function defer() {
    try {
      const next = await review.transition("defer", selected.media_id);
      if (next.current_media_id && review.isActive())
        onNavigate?.(next.current_media_id);
    } catch {
      /* The hook owns the visible error. */
    }
  }
  const unavailable =
    selected.reason === "missing_media" ||
    selected.reason === "unavailable_media";
  return (
    <section className="panel mb-6 space-y-4" aria-label={t("reviewQueues")}>
      <p aria-live="polite" className="text-sm">
        {t("reviewSummary", { count: queue.items.length, ...counts })}
      </p>
      <div className="flex flex-wrap gap-2">
        {queue.items.map((item) => (
          <Button
            key={item.media_id}
            variant={
              item.media_id === selected.media_id ? "default" : "outline"
            }
            size="sm"
            disabled={busy || !onNavigate}
            aria-current={
              item.media_id === selected.media_id ? "step" : undefined
            }
            aria-label={t("reviewSelect", {
              index: item.position + 1,
              state: t(`reviewState_${item.state}`),
            })}
            onClick={() => void choose(item.media_id)}
          >
            {item.position + 1} · {t(`reviewState_${item.state}`)}
          </Button>
        ))}
      </div>
      <div className="flex flex-wrap gap-2">
        {selected.state === "pending" &&
          queue.current_media_id === selected.media_id &&
          !selected.needs_recheck && (
            <Button
              size="sm"
              variant="outline"
              disabled={busy}
              onClick={() => void defer()}
            >
              {t("reviewDefer")}
            </Button>
          )}
        {!unavailable &&
          (selected.state === "failed" ||
            selected.state === "deferred" ||
            (selected.state === "pending" &&
              (selected.needs_recheck ||
                queue.current_media_id !== selected.media_id))) && (
            <Button
              size="sm"
              disabled={busy}
              onClick={() =>
                void review
                  .transition(
                    selected.state === "pending" ? "select" : "retry",
                    selected.media_id,
                  )
                  .catch(() => {})
              }
            >
              {t("reviewRetry")}
            </Button>
          )}
        <Button
          size="sm"
          variant="ghost"
          disabled={busy}
          onClick={review.refresh}
        >
          {t("reviewRefresh")}
        </Button>
        <Button asChild variant="outline" size="sm" disabled={busy}>
          {busy ? (
            <span>{t("scraperQueueReturn")}</span>
          ) : (
            <Link
              to="/tools"
              search={validateToolsSearch({ ...queue.context })}
            >
              {t("scraperQueueReturn")}
            </Link>
          )}
        </Button>
      </div>
      {(selected.needs_recheck ||
        selected.state === "failed" ||
        selected.state === "deferred") && (
        <p role="status" className="notice">
          {t(
            unavailable
              ? "reviewUnavailable"
              : selected.needs_recheck
                ? "reviewRecheck"
                : selected.state === "failed"
                  ? "reviewFailed"
                  : "reviewDeferred",
          )}
        </p>
      )}
      {queue.current_media_id === null && (
        <p role="status" className="text-sm text-muted-foreground">
          {t("reviewFinished")}
        </p>
      )}
      {review.error && (
        <ErrorNotice error={review.error} retry={review.refresh} />
      )}
    </section>
  );
}
