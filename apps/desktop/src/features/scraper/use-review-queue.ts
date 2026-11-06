import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useRef, useState } from "react";
import { DesktopError } from "../../api/desktop";
import {
  type ReviewQueue,
  type ReviewUpdate,
  reviewQueueApi,
} from "../../api/review-queue";
import { scraperApi } from "../../api/scraper";
import { reviewCanWork, reviewMediaId, reviewQueueQuery } from "./queries";

/** A session only restores data. Every write below requires an explicit user action. */
export function useReviewQueue(sessionId: string, mediaId?: string) {
  const client = useQueryClient();
  const options = reviewQueueQuery(sessionId);
  const key = options.queryKey;
  const query = useQuery(options);
  const latest = useRef(query.data);
  latest.current = query.data;
  const active = useRef(true);
  const scope = `${sessionId}:${mediaId ?? ""}`;
  const currentScope = useRef(scope);
  currentScope.current = scope;
  const isActive = () => active.current && currentScope.current === scope;
  const locked = useRef(false);
  const [confirmedHere, setConfirmedHere] = useState<string>();
  useEffect(() => {
    active.current = true;
    return () => {
      active.current = false;
    };
  }, []);
  function saved(value: ReviewQueue) {
    latest.current = value;
    client.setQueryData(key, value);
    void client.invalidateQueries({ queryKey: ["review-queues"] });
    return value;
  }
  function current() {
    if (!latest.current || locked.current || !isActive())
      throw new DesktopError("request_failed");
    return latest.current;
  }
  async function update(action: ReviewUpdate["action"], id: string) {
    const queue = current();
    locked.current = true;
    try {
      return saved(
        await reviewQueueApi.update({
          queue_id: queue.id,
          media_id: id,
          expected_revision: queue.revision,
          action,
        }),
      );
    } finally {
      locked.current = false;
    }
  }
  const transition = useMutation({
    mutationFn: ({
      action,
      id,
    }: {
      action: ReviewUpdate["action"];
      id: string;
    }) => update(action, id),
  });
  const selectedId = reviewMediaId(query.data, mediaId);
  const selected = query.data?.items.find(
    (item) => item.media_id === selectedId,
  );
  const confirmation = useMutation({
    mutationFn: async (previewId: string) => {
      const queue = current();
      if (!selectedId || queue.current_media_id !== selectedId)
        throw new DesktopError("request_failed");
      locked.current = true;
      let applied = false;
      try {
        const checked = saved(
          await reviewQueueApi.checkpoint({
            queue_id: queue.id,
            media_id: selectedId,
            expected_revision: queue.revision,
            preview_id: previewId,
          }),
        );
        const checkpoint = checked.items.find(
          (item) => item.media_id === selectedId,
        )?.checkpoint;
        if (!checkpoint || !isActive())
          throw new DesktopError("request_failed");
        const result = await scraperApi.confirm(previewId);
        applied = true;
        if (isActive()) setConfirmedHere(selectedId);
        saved(
          await reviewQueueApi.update({
            queue_id: checked.id,
            media_id: selectedId,
            expected_revision: checked.revision,
            action: "confirmed",
            checkpoint,
          }),
        );
        return result;
      } catch (error) {
        // A ledger failure must never relabel already applied metadata as failed.
        if (!applied && latest.current?.current_media_id === selectedId) {
          const snapshot = latest.current;
          try {
            saved(
              await reviewQueueApi.update({
                queue_id: snapshot.id,
                media_id: selectedId,
                expected_revision: snapshot.revision,
                action: "failed",
              }),
            );
          } catch {
            /* Keep original error; explicit refresh resolves conflicts. */
          }
        }
        throw error;
      } finally {
        locked.current = false;
      }
    },
  });
  function failed() {
    if (
      !selectedId ||
      locked.current ||
      !isActive() ||
      latest.current?.current_media_id !== selectedId
    )
      return;
    transition.mutate({ action: "failed", id: selectedId });
  }
  return {
    query,
    selected,
    selectedId,
    isActive,
    canWork: reviewCanWork(query.data, selectedId, confirmedHere),
    busy: transition.isPending || confirmation.isPending,
    error: transition.error ?? confirmation.error,
    transition: (action: ReviewUpdate["action"], id: string) =>
      transition.mutateAsync({ action, id }),
    confirm: (previewId: string) => confirmation.mutateAsync(previewId),
    failed,
    refresh: () => {
      transition.reset();
      confirmation.reset();
      void query.refetch();
    },
  };
}
export type ReviewSession = ReturnType<typeof useReviewQueue>;
