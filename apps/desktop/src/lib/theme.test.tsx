import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { Button } from "../components/ui/button";
import {
  initializeTheme,
  readStoredTheme,
  setTheme,
  THEME_STORAGE_KEY,
  useTheme,
} from "./theme";

function ThemeControl() {
  const theme = useTheme();
  return (
    <Button onClick={() => setTheme(theme === "dark" ? "light" : "dark")}>
      {theme}
    </Button>
  );
}

beforeEach(() => {
  localStorage.clear();
  initializeTheme();
});
afterEach(() => {
  vi.restoreAllMocks();
  localStorage.clear();
  initializeTheme();
});

it("restores only supported themes and defaults to dark", () => {
  expect(readStoredTheme()).toBe("dark");
  localStorage.setItem(THEME_STORAGE_KEY, "unsupported");
  expect(readStoredTheme()).toBe("dark");
  localStorage.setItem(THEME_STORAGE_KEY, "light");
  initializeTheme();
  expect(document.documentElement).toHaveAttribute("data-theme", "light");
  expect(document.documentElement).not.toHaveClass("dark");
  expect(document.documentElement.style.colorScheme).toBe("light");
});

it("updates all mounted controls immediately and persists the selection", () => {
  render(
    <>
      <ThemeControl />
      <ThemeControl />
    </>,
  );
  fireEvent.click(screen.getAllByRole("button", { name: "dark" })[0]);
  expect(screen.getAllByRole("button", { name: "light" })).toHaveLength(2);
  expect(localStorage.getItem(THEME_STORAGE_KEY)).toBe("light");
  fireEvent.click(screen.getAllByRole("button", { name: "light" })[1]);
  expect(screen.getAllByRole("button", { name: "dark" })).toHaveLength(2);
  expect(document.documentElement).toHaveClass("dark");
  expect(localStorage.getItem(THEME_STORAGE_KEY)).toBe("dark");
});

it("keeps switching usable when storage access fails", () => {
  vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
    throw new Error("Storage unavailable");
  });
  vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
    throw new Error("Storage unavailable");
  });
  expect(readStoredTheme()).toBe("dark");
  render(<ThemeControl />);
  fireEvent.click(screen.getByRole("button", { name: "dark" }));
  expect(screen.getByRole("button", { name: "light" })).toBeInTheDocument();
  expect(document.documentElement).not.toHaveClass("dark");
});

it("syncs saved changes and storage clearing from another window", () => {
  render(<ThemeControl />);
  localStorage.setItem(THEME_STORAGE_KEY, "light");
  fireEvent(window, new StorageEvent("storage", { key: "unrelated" }));
  expect(screen.getByRole("button", { name: "dark" })).toBeInTheDocument();
  fireEvent(window, new StorageEvent("storage", { key: THEME_STORAGE_KEY }));
  expect(screen.getByRole("button", { name: "light" })).toBeInTheDocument();
  localStorage.clear();
  fireEvent(window, new StorageEvent("storage", { key: null }));
  expect(screen.getByRole("button", { name: "dark" })).toBeInTheDocument();
});
