import { isTauri } from "@tauri-apps/api/core";
import type { UnlistenFn } from "@tauri-apps/api/event";
import {
  getCurrentWindow,
  type Window as NativeWindow,
} from "@tauri-apps/api/window";
import { useEffect, useRef, useState } from "react";

export type WindowPlatform = "macos" | "windows" | "linux";
type Session = { platform: WindowPlatform; window: NativeWindow };
type Operation = "minimize" | "toggleMaximize" | "close";

function currentSession(linuxControls: boolean): Session | null {
  if (!isTauri()) return null;
  const identity = navigator.platform || navigator.userAgent;
  const platform = /Mac/i.test(identity)
    ? "macos"
    : /Win/i.test(identity)
      ? "windows"
      : linuxControls && /Linux/i.test(identity)
        ? "linux"
        : null;
  if (!platform) return null;
  try {
    const window = getCurrentWindow();
    return window.label === "main" || window.label.startsWith("nfo-")
      ? { platform, window }
      : null;
  } catch {
    // Browser IPC fixtures do not have native window metadata.
    return null;
  }
}

export function useWindowChrome({ linuxControls = false }: { linuxControls?: boolean } = {}) {
  const [session] = useState(() => currentSession(linuxControls));
  const [attempt, setAttempt] = useState(0);
  const attempts = useRef(0);
  const [state, setState] = useState({
    ready: session?.platform === "macos",
    maximized: false,
    focused: true,
    busy: false,
    error: false,
  });
  const scope = useRef(0);
  const refresh = useRef<(() => Promise<void>) | null>(null);
  const locked = useRef(false);
  const latestReady = useRef(state.ready);
  latestReady.current = state.ready;

  useEffect(() => {
    const epoch = ++scope.current;
    if (!session || session.platform === "macos") return;
    const native = session.window;
    const unlisten: UnlistenFn[] = [];
    let alive = true;
    let revision = 0;
    let listenerFailed = false;
    const current = () =>
      alive && scope.current === epoch && attempts.current === attempt;
    async function readState() {
      if (!current()) return;
      const request = ++revision;
      try {
        const [maximized, focused] = await Promise.all([
          native.isMaximized(),
          native.isFocused(),
        ]);
        if (!current() || request !== revision) return;
        if (typeof maximized !== "boolean" || typeof focused !== "boolean")
          throw new Error("Invalid native window state");
        setState((old) => ({
          ...old,
          ready: true,
          maximized,
          focused,
          error: listenerFailed,
        }));
      } catch {
        if (current() && request === revision)
          setState((old) => ({ ...old, error: true }));
      }
    }
    refresh.current = readState;
    setState((old) => ({ ...old, ready: false, error: false }));
    function retain(registration: Promise<UnlistenFn>) {
      void registration.then(
        (off) => {
          if (current()) unlisten.push(off);
          else off();
        },
        () => {
          listenerFailed = true;
          if (current()) setState((old) => ({ ...old, error: true }));
        },
      );
    }
    retain(
      native.onResized(() => {
        void readState();
      }),
    );
    retain(
      native.onFocusChanged(() => {
        void readState();
      }),
    );
    void readState();
    return () => {
      alive = false;
      if (scope.current === epoch) scope.current += 1;
      revision += 1;
      refresh.current = null;
      for (const off of unlisten) off();
    };
  }, [session, attempt]);

  async function perform(operation: Operation) {
    const read = refresh.current;
    if (
      !session || session.platform === "macos" ||
      !latestReady.current ||
      locked.current ||
      !read
    )
      return;
    const epoch = scope.current;
    locked.current = true;
    setState((old) => ({ ...old, busy: true, error: false }));
    try {
      await session.window[operation]();
      if (scope.current === epoch) await read();
    } catch {
      if (scope.current === epoch) setState((old) => ({ ...old, error: true }));
    } finally {
      locked.current = false;
      if (scope.current === epoch) setState((old) => ({ ...old, busy: false }));
    }
  }

  return {
    platform: session?.platform ?? null,
    ...state,
    minimize: () => perform("minimize"),
    toggleMaximize: () => perform("toggleMaximize"),
    close: () => perform("close"),
    retry: () => {
      attempts.current += 1;
      setAttempt(attempts.current);
    },
  };
}

export type WindowChrome = ReturnType<typeof useWindowChrome>;
