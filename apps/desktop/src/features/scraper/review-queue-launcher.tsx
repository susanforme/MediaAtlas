import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link, useNavigate } from "@tanstack/react-router";
import { ChevronRight } from "lucide-react";
import { type ReactNode, useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import { reviewQueueApi } from "../../api/review-queue";
import { Badge } from "../../components/ui/badge";
import { Button } from "../../components/ui/button";
import { Details, DetailsSummary } from "../../components/ui/details";
import { ErrorNotice, useFormat } from "../library/shared";
import { type ToolsRouteSearch, validateToolsSearch } from "../tools/search";
import { MAX_SCRAPE_QUEUE } from "./search";
import { reviewQueuesQuery } from "./queries";
import "./review-queue-translations";

export function ReviewQueueLauncher({
  mediaIds,
  context,
  children,
}: {
  mediaIds: string[];
  context: ToolsRouteSearch;
  children?: ReactNode;
}) {
  const { t } = useTranslation();
  const { date } = useFormat();
  const navigate = useNavigate();
  const client = useQueryClient();
  const active = useRef(true);
  const lock = useRef(false);
  const latestContext = useRef(context);
  latestContext.current = context;
  useEffect(() => {
    active.current = true;
    return () => {
      active.current = false;
    };
  }, []);
  const sessions = useQuery(reviewQueuesQuery);
  const create = useMutation({
    mutationFn: async () => {
      if (
        lock.current ||
        !mediaIds.length ||
        mediaIds.length > MAX_SCRAPE_QUEUE
      )
        return;
      lock.current = true;
      const started = JSON.stringify(context);
      try {
        const filters = validateToolsSearch({ ...context, media: undefined });
        const queue = await reviewQueueApi.create(mediaIds, filters);
        client.setQueryData(["review-queue", queue.id], queue);
        await client.invalidateQueries({ queryKey: ["review-queues"] });
        if (active.current && started === JSON.stringify(latestContext.current))
          await navigate({
            to: "/scraper",
            search: {
              session: queue.id,
              media: queue.current_media_id ?? queue.items[0]?.media_id,
            },
          });
      } finally {
        lock.current = false;
      }
    },
  });
  return (
    <div className="space-y-3">
      {mediaIds.length > 0 && <div data-slot="selection-toolbar" role="region" aria-label={t("toolsSelected", { count: mediaIds.length })} className="flex flex-wrap items-center gap-3 rounded-xl border border-primary/30 bg-primary/10 px-4 py-3">
        {children}
        <Button
          size="sm"
          disabled={!mediaIds.length || create.isPending}
          onClick={() => create.mutate()}
        >
          {t("toolsScrapeSelected")}
        </Button>
      </div>}
      {create.error && <ErrorNotice error={create.error} />}
      {sessions.error && (
        <ErrorNotice
          error={sessions.error}
          retry={() => void sessions.refetch()}
        />
      )}
      {Boolean(sessions.data?.length) && (
        <section aria-label={t("reviewQueues")}>
          <Details open className="group/queues rounded-lg border border-border bg-surface/30 px-3 py-2">
            <DetailsSummary className="flex items-center gap-2 text-xs text-muted-foreground">
              <ChevronRight size={14} className="shrink-0 transition-transform group-open/queues:rotate-90" aria-hidden="true" />
              {t("reviewQueues")}
              <Badge variant="outline" className="ml-auto">{sessions.data?.length}</Badge>
            </DetailsSummary>
            <div className="mt-2 divide-y divide-border">
          {sessions.data?.map((queue) => (
            <div
              key={queue.id}
              className="flex flex-wrap items-center justify-between gap-x-4 gap-y-2 py-2 text-xs"
            >
              <div className="flex min-w-0 flex-wrap items-center gap-x-3 gap-y-1">
                <span className="text-muted-foreground">{date(queue.updated_at)}</span>
                <span>{t("reviewItemCount", { count: queue.items.length })}</span>
                {(["confirmed", "deferred", "failed"] as const).map((state) => {
                  const count = queue.items.filter((item) => item.state === state).length;
                  return count > 0 ? <Badge key={state} variant={state === "failed" ? "destructive" : "secondary"}>{t(`reviewCount_${state}`, { count })}</Badge> : null;
                })}
              </div>
              <Button asChild variant="ghost" size="sm">
                <Link
                  to="/scraper"
                  search={{
                    session: queue.id,
                    media:
                      queue.current_media_id ??
                      queue.items.find((item) => item.state !== "confirmed")
                        ?.media_id ??
                      queue.items[0]?.media_id,
                  }}
                >
                  {t("reviewResume")}
                </Link>
              </Button>
            </div>
          ))}
            </div>
          </Details>
        </section>
      )}
    </div>
  );
}
