import { useQueryClient } from "@tanstack/react-query";
import { isTauri } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useEffect } from "react";

export function useTaskEvents() {
  const client = useQueryClient();
  useEffect(() => {
    if (!isTauri()) return;
    let closed = false;
    const subscriptions: UnlistenFn[] = [];
    for (const event of [
      "task-created",
      "task-updated",
      "task-needs-review",
      "task-completed",
    ]) {
      void listen<{ schema_version: number }>(event, (message) => {
        if (message.payload.schema_version !== 1) return;
        for (const key of [
          "tasks",
          "preview",
          "summary",
          "media",
          "facets",
          "roots",
        ])
          void client.invalidateQueries({ queryKey: [key] });
      })
        .then((unlisten) => {
          if (closed) unlisten();
          else subscriptions.push(unlisten);
        })
        .catch(() => {
          /* Database polling remains authoritative. */
        });
    }
    return () => {
      closed = true;
      for (const unlisten of subscriptions) unlisten();
    };
  }, [client]);
}
