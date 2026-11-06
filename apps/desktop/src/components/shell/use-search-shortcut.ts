import { useEffect } from "react";

export function useSearchShortcut() {
  useEffect(() => {
    function focusSearch(event: KeyboardEvent) {
      if (
        event.defaultPrevented || event.isComposing || event.altKey || event.shiftKey ||
        !(event.ctrlKey || event.metaKey) || event.key.toLowerCase() !== "f" ||
        document.querySelector('[role="dialog"], [role="alertdialog"]')
      ) return;
      const input = document.querySelector<HTMLInputElement>(
        '[data-slot="app-toolbar"] [role="search"] input[type="search"]',
      );
      if (!input || input.disabled) return;
      event.preventDefault();
      input.focus();
      input.select();
    }
    window.addEventListener("keydown", focusSearch);
    return () => window.removeEventListener("keydown", focusSearch);
  }, []);
}
