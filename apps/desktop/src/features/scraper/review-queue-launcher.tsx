import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link, useNavigate } from "@tanstack/react-router";
import { type ReactNode, useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import { reviewQueueApi } from "../../api/review-queue";
import { Button } from "../../components/ui/button";
import { ErrorNotice, useFormat } from "../library/shared";
import { type ToolsRouteSearch, validateToolsSearch } from "../tools/search";
import { MAX_SCRAPE_QUEUE } from "./search";
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
  const sessions = useQuery({
    queryKey: ["review-queues"],
    queryFn: reviewQueueApi.list,
    retry: false,
  });
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
      <div className="flex flex-wrap items-center gap-2">
        {children}
        <Button
          size="sm"
          disabled={!mediaIds.length || create.isPending}
          onClick={() => create.mutate()}
        >
          {t("toolsScrapeSelected")}
        </Button>
      </div>
      {create.error && <ErrorNotice error={create.error} />}
      {sessions.error && (
        <ErrorNotice
          error={sessions.error}
          retry={() => void sessions.refetch()}
        />
      )}
      {Boolean(sessions.data?.length) && (
        <section aria-label={t("reviewQueues")} className="space-y-2">
          <h3 className="text-sm font-medium">{t("reviewQueues")}</h3>
          {sessions.data?.map((queue) => (
            <div
              key={queue.id}
              className="flex flex-wrap items-center justify-between gap-3 text-sm"
            >
              <p>
                {date(queue.updated_at)} ·{" "}
                {t("reviewSummary", {
                  count: queue.items.length,
                  confirmed: queue.items.filter(
                    (item) => item.state === "confirmed",
                  ).length,
                  deferred: queue.items.filter(
                    (item) => item.state === "deferred",
                  ).length,
                  failed: queue.items.filter((item) => item.state === "failed")
                    .length,
                })}
              </p>
              <Button asChild variant="outline" size="sm">
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
        </section>
      )}
    </div>
  );
}
