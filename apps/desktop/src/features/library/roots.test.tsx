import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { I18nextProvider } from "react-i18next";
import { beforeEach, expect, it, vi } from "vitest";
import { libraryApi, type RootInput, type RootView } from "../../api/library";
import i18n from "../../i18n";
import { PrivacyProvider } from "./context";
import { RootsPage } from "./pages";

vi.mock("@tanstack/react-router", () => ({
  Link: ({ to, children }: { to: string; children: React.ReactNode }) => (
    <a href={to}>{children}</a>
  ),
}));
vi.mock("../../api/library", async (original) => {
  const module = await original<typeof import("../../api/library")>();
  return {
    ...module,
    libraryApi: { ...module.libraryApi, roots: vi.fn(), saveRoot: vi.fn() },
  };
});
const time = "2026-11-03T00:00:00Z";
function show() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  render(
    <I18nextProvider i18n={i18n}>
      <QueryClientProvider client={client}>
        <PrivacyProvider>
          <RootsPage />
        </PrivacyProvider>
      </QueryClientProvider>
    </I18nextProvider>,
  );
}
beforeEach(async () => {
  localStorage.clear();
  await i18n.changeLanguage("en-US");
  vi.mocked(libraryApi.roots).mockResolvedValue([]);
  vi.mocked(libraryApi.saveRoot).mockImplementation(
    async (input: RootInput) =>
      ({
        ...input,
        id: "fixture-root",
        path_style: "posix",
        created_at: time,
        updated_at: time,
        last_scan_at: null,
      }) as RootView,
  );
});
it("submits an explicit root type and normalized exclusion lines", async () => {
  show();
  await screen.findByText("No library roots yet");
  fireEvent.click(screen.getByRole("button", { name: "Add root" }));
  fireEvent.change(screen.getByLabelText("Root name"), {
    target: { value: "Fixture root" },
  });
  fireEvent.change(screen.getByLabelText("Absolute path"), {
    target: { value: "/fixture/media" },
  });
  fireEvent.change(
    screen.getByLabelText("Exclude patterns (one glob per line)"),
    { target: { value: "  **/sample/**\n\n**/*.tmp  " } },
  );
  fireEvent.click(screen.getByRole("button", { name: "Save" }));
  await waitFor(() =>
    expect(libraryApi.saveRoot).toHaveBeenCalledWith({
      id: null,
      name: "Fixture root",
      path: "/fixture/media",
      kind: "movie",
      enabled: true,
      exclude_patterns: ["**/sample/**", "**/*.tmp"],
    }),
  );
});
it("uses localized validation instead of submitting empty root fields", async () => {
  show();
  await screen.findByText("No library roots yet");
  fireEvent.click(screen.getByRole("button", { name: "Add root" }));
  fireEvent.click(screen.getByRole("button", { name: "Save" }));
  expect(await screen.findByRole("alert")).toHaveTextContent(
    "Enter a root name, absolute path and media type.",
  );
  expect(libraryApi.saveRoot).not.toHaveBeenCalled();
});

it("keeps root removal explicit and restores focus when its dialog is dismissed", async () => {
  vi.mocked(libraryApi.roots).mockResolvedValue([
    {
      id: "fixture-root",
      name: "Synthetic directory",
      path: "/fixture/media",
      path_style: "posix",
      kind: "movie",
      enabled: true,
      exclude_patterns: [],
      created_at: time,
      updated_at: time,
      last_scan_at: null,
    },
  ]);
  show();
  const trigger = await screen.findByRole("button", { name: "Remove" });
  trigger.focus();
  fireEvent.click(trigger);
  const dialog = await screen.findByRole("dialog", {
    name: "Remove root index",
  });
  expect(dialog).toHaveTextContent("Source files on disk will be preserved.");
  await waitFor(() =>
    expect(dialog).toContainElement(document.activeElement as HTMLElement),
  );
  fireEvent.keyDown(dialog, { key: "Escape" });
  await waitFor(() =>
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument(),
  );
  expect(trigger).toHaveFocus();
});
