import type { ReactNode } from "react";
import { useWindowChrome } from "./use-window-controls";
import { WindowControls } from "./window-controls";

export function StandaloneWindow({ children, linuxControls = false }: {
  children: ReactNode;
  linuxControls?: boolean;
}) {
  const chrome = useWindowChrome({ linuxControls });
  return (
    <div data-slot="standalone-window" className="h-dvh overflow-hidden overscroll-none">
      <header
        data-slot="window-titlebar"
        className="fixed inset-x-0 top-0 z-40 h-9 select-none bg-background/70 backdrop-blur-xl data-[window-platform=macos]:h-12"
        data-window-platform={chrome.platform ?? undefined}
        data-tauri-drag-region={chrome.platform ? "deep" : undefined}
      >
        <WindowControls chrome={chrome} />
      </header>
      <main className="mx-auto h-full max-w-[1440px] overflow-x-hidden overflow-y-auto overscroll-y-contain scroll-pt-16 px-6 pt-20 pb-8 sm:px-10 sm:pb-10">
        {children}
      </main>
    </div>
  );
}
