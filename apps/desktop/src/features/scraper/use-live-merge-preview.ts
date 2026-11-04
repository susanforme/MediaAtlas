import { useEffect, useState } from "react";
import {
  type MergePreview,
  type MergeRequest,
  scraperApi,
} from "../../api/scraper";

type PreviewState = {
  signature: string | null;
  attempt: number;
} & (
  | { status: "idle" | "pending" }
  | { status: "ready"; preview: MergePreview }
  | { status: "error"; error: unknown }
);

/** Read-only validation follows the current selection; confirmation stays explicit. */
export function useLiveMergePreview(request: MergeRequest | null) {
  // Serialize the contract in a fixed order so new object identities do not fetch.
  const signature = request
    ? JSON.stringify({
        run_id: request.run_id,
        candidate_index: request.candidate_index,
        media_item_id: request.media_item_id,
        selected_fields: request.selected_fields,
        selected_artwork: request.selected_artwork,
      })
    : null;
  const [attempt, setAttempt] = useState(0);
  const [state, setState] = useState<PreviewState>({
    signature: null,
    attempt: 0,
    status: "idle",
  });

  useEffect(() => {
    if (signature === null) {
      setState({ signature, attempt, status: "idle" });
      return;
    }
    let active = true;
    setState({ signature, attempt, status: "pending" });
    const timer = setTimeout(() => {
      void scraperApi.preview(JSON.parse(signature) as MergeRequest).then(
        (preview) => {
          if (active)
            setState({ signature, attempt, status: "ready", preview });
        },
        (error: unknown) => {
          if (active) setState({ signature, attempt, status: "error", error });
        },
      );
    }, 200);
    return () => {
      active = false;
      clearTimeout(timer);
    };
  }, [signature, attempt]);

  const current =
    signature !== null &&
    state.signature === signature &&
    state.attempt === attempt;
  return {
    preview: current && state.status === "ready" ? state.preview : undefined,
    pending: signature !== null && (!current || state.status === "pending"),
    error: current && state.status === "error" ? state.error : undefined,
    retry: () => setAttempt((previous) => previous + 1),
  };
}
