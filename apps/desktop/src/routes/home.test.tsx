import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen } from "@testing-library/react";
import { I18nextProvider } from "react-i18next";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { DesktopError, getAppInfo } from "../api/desktop";
import i18n from "../i18n";
import { HomePage } from "./home";

vi.mock("../api/desktop", async (importOriginal) => {
  const original = await importOriginal<typeof import("../api/desktop")>();
  return { ...original, getAppInfo: vi.fn() };
});

function renderPage() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <I18nextProvider i18n={i18n}>
      <QueryClientProvider client={client}>
        <HomePage />
      </QueryClientProvider>
    </I18nextProvider>,
  );
}

describe("desktop connection screen", () => {
  beforeEach(async () => {
    await i18n.changeLanguage("en");
  });

  it("explains browser preview without displaying a connected state", async () => {
    vi.mocked(getAppInfo).mockRejectedValue(
      new DesktopError("desktop_required"),
    );
    renderPage();
    expect(
      await screen.findByText("Open in the desktop app"),
    ).toBeInTheDocument();
    expect(screen.queryByText("Local core connected")).not.toBeInTheDocument();
  });

  it("shows the actual backend version", async () => {
    vi.mocked(getAppInfo).mockResolvedValue({
      schema_version: 1,
      app_name: "MediaAtlas",
      app_version: "0.8.1",
    });
    renderPage();
    expect(await screen.findByText("Local core connected")).toBeInTheDocument();
    expect(screen.getByText("App version 0.8.1")).toBeInTheDocument();
  });

  it("allows recovery after a failed connection", async () => {
    vi.mocked(getAppInfo)
      .mockRejectedValueOnce(new DesktopError("request_failed"))
      .mockResolvedValueOnce({
        schema_version: 1,
        app_name: "MediaAtlas",
        app_version: "0.1.0",
      });
    renderPage();
    expect(
      await screen.findByText(
        "Could not connect to the local core. Please try again.",
      ),
    ).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Check again" }));
    expect(await screen.findByText("Local core connected")).toBeInTheDocument();
  });
});
