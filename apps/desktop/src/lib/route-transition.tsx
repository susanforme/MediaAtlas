import type { QueryClient } from "@tanstack/react-query";
import { type ErrorComponentProps, useRouter, useRouterState } from "@tanstack/react-router";
import type { ReactElement, ReactNode } from "react";
import type { AdultSettings } from "../api/adult-privacy";
import { ErrorNotice, Loading } from "../features/library/shared";

export interface RouteDataContext {
  queryClient: QueryClient;
  readSettings: () => AdultSettings | null;
}

export interface RoutePreparation extends RouteDataContext {
  check: () => void;
}

const preparing = new Set<AbortController>();

export function cancelRoutePreparations() {
  for (const controller of preparing) controller.abort();
  preparing.clear();
}

export async function prepareDesktopRoute(
  controller: AbortController,
  load: () => Promise<unknown>,
  component: { preload?: () => Promise<unknown> | undefined },
  data: (check: () => void) => Promise<unknown>,
) {
  let active = true;
  const check = () => {
    if (!active || controller.signal.aborted) throw new DOMException("Navigation cancelled", "AbortError");
  };
  check();
  preparing.add(controller);
  try {
    await Promise.all([
      load().then(() => { check(); return component.preload?.(); }),
      data(check),
    ]);
    check();
  } finally {
    active = false;
    preparing.delete(controller);
  }
}

export function InitialRouteLoading({ children }: { children: ReactNode }): ReactElement {
  const initial: boolean = useRouterState({ select: (state) => state.matches.length === 0 });
  return <>{initial && <Loading />}{children}</>;
}

export function RouteFailure({ error, reset }: ErrorComponentProps): ReactElement {
  const router = useRouter();
  return <ErrorNotice error={error} retry={() => { void router.invalidate({ sync: true }).then(reset); }} />;
}
