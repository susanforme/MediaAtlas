import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { expect, it, vi } from "vitest";
import { isNfoStartup } from "./api/nfo";
import i18n from "./i18n";

const startupRender = vi.hoisted(() => vi.fn<(node: ReactNode) => void>());
vi.mock("react-dom/client", async (importOriginal) => {
  const actual = await importOriginal<typeof import("react-dom/client")>();
  return {
    ...actual,
    createRoot: (...args: Parameters<typeof actual.createRoot>) =>
      args[0] instanceof HTMLElement && args[0].id === "root"
        ? { render: startupRender }
        : actual.createRoot(...args),
  };
});
vi.mock("./api/nfo", () => ({ isNfoStartup: vi.fn() }));
vi.mock("./app", () => ({ App: () => <h1>Library ready</h1> }));
vi.mock("./features/nfo/page", () => ({ NfoStandalone: () => <h1>NFO ready</h1> }));
vi.mock("./lib/theme", () => ({ initializeTheme: vi.fn() }));
vi.mock("./components/shell/standalone-window", () => ({
  StandaloneWindow: ({ children }: { children: ReactNode }) => <section data-testid="window-frame">{children}</section>,
}));

it("keeps a window frame on startup failure and retries mode selection without reloading", async () => {
  await i18n.changeLanguage("en-US");
  vi.mocked(isNfoStartup).mockRejectedValueOnce(new Error("private startup path"))
    .mockResolvedValueOnce(false);
  const root = document.createElement("div");
  root.id = "root";
  document.body.append(root);
  try {
    await import("./main");
    await waitFor(() => expect(startupRender).toHaveBeenCalledTimes(1));
    const view = render(startupRender.mock.calls[0][0]);
    expect(screen.getByTestId("window-frame")).toContainElement(screen.getByRole("alert"));
    expect(screen.queryByText("private startup path")).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Check again" }));
    await waitFor(() => expect(startupRender).toHaveBeenCalledTimes(2));
    expect(isNfoStartup).toHaveBeenCalledTimes(2);
    view.rerender(startupRender.mock.calls[1][0]);
    expect(screen.getByRole("heading", { name: "Library ready" })).toBeInTheDocument();
  } finally {
    root.remove();
  }
});
