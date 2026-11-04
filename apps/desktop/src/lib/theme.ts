import { useSyncExternalStore } from "react";

export type Theme = "light" | "dark";
export const THEME_STORAGE_KEY = "mediaatlas.theme";
const THEME_CHANGE = "mediaatlas-theme-change";

export function readStoredTheme(): Theme {
  try {
    return localStorage.getItem(THEME_STORAGE_KEY) === "light" ? "light" : "dark";
  } catch {
    return "dark";
  }
}

function applyTheme(theme: Theme) {
  const root = document.documentElement;
  root.classList.toggle("dark", theme === "dark");
  root.dataset.theme = theme;
  root.style.colorScheme = theme;
  document
    .querySelector('meta[name="theme-color"]')
    ?.setAttribute("content", theme === "dark" ? "#101016" : "#f7f7fb");
}

export function initializeTheme() {
  applyTheme(readStoredTheme());
}

export function setTheme(theme: Theme) {
  applyTheme(theme);
  try {
    localStorage.setItem(THEME_STORAGE_KEY, theme);
  } catch {
    // Keep switching available when storage is unavailable.
  }
  window.dispatchEvent(new Event(THEME_CHANGE));
}

function subscribe(listener: () => void) {
  function syncStorage(event: StorageEvent) {
    if (event.key !== THEME_STORAGE_KEY && event.key !== null) return;
    initializeTheme();
    listener();
  }
  window.addEventListener(THEME_CHANGE, listener);
  window.addEventListener("storage", syncStorage);
  return () => {
    window.removeEventListener(THEME_CHANGE, listener);
    window.removeEventListener("storage", syncStorage);
  };
}

function getTheme(): Theme {
  return document.documentElement.dataset.theme === "light" ? "light" : "dark";
}

export function useTheme() {
  return useSyncExternalStore(subscribe, getTheme, () => "dark" as const);
}
