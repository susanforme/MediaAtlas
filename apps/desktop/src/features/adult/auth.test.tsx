import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen } from "@testing-library/react";
import { I18nextProvider } from "react-i18next";
import { beforeEach, expect, it, vi } from "vitest";
import { type AuthStatus, providerAuthApi } from "../../api/provider-auth";
import i18n from "../../i18n";
import { ProviderAuthSettings } from "../settings/provider-auth";

const privacy = vi.hoisted(() => ({ includeAdult: true }));
vi.mock("../library/context", () => ({ usePrivacy: () => privacy }));
vi.mock("../../api/provider-auth", async (original) => ({
  ...(await original<typeof import("../../api/provider-auth")>()),
  providerAuthApi: { status: vi.fn(), save: vi.fn(), clear: vi.fn() },
}));
const status: AuthStatus = {
  provider_id: "dmm",
  supported_methods: ["affiliate"],
  method: "affiliate",
  storage: "system",
  configured: false,
  status: "not_configured",
};
function show() {
  return render(
    <I18nextProvider i18n={i18n}>
      <QueryClientProvider
        client={
          new QueryClient({ defaultOptions: { queries: { retry: false } } })
        }
      >
        <ProviderAuthSettings />
      </QueryClientProvider>
    </I18nextProvider>,
  );
}
beforeEach(async () => {
  vi.clearAllMocks();
  localStorage.clear();
  privacy.includeAdult = true;
  await i18n.changeLanguage("en-US");
  vi.mocked(providerAuthApi.status).mockImplementation(async (provider) =>
    provider === "dmm"
      ? status
      : {
          ...status,
          provider_id: provider,
          supported_methods:
            provider === "tmdb" ? ["bearer", "api_key"] : ["none"],
          method: provider === "tmdb" ? "bearer" : "none",
          configured: provider !== "tmdb",
          status: provider === "tmdb" ? "not_configured" : "not_required",
        },
  );
  vi.mocked(providerAuthApi.save).mockImplementation(async (input) => ({
    ...status,
    storage: input.storage,
    configured: true,
    status: "configured",
  }));
});
it("stores both masked DMM fields as one inbound credential and clears them on submission", async () => {
  const stored = vi.spyOn(Storage.prototype, "setItem");
  show();
  fireEvent.change(screen.getByLabelText("Authentication provider"), {
    target: { value: "dmm" },
  });
  const api = await screen.findByLabelText("DMM API ID");
  const affiliate = screen.getByLabelText("DMM affiliate ID");
  expect(api).toHaveAttribute("type", "password");
  expect(affiliate).toHaveAttribute("type", "password");
  expect(screen.getByLabelText("Credential storage")).toHaveValue("system");
  fireEvent.change(api, { target: { value: "synthetic-api" } });
  expect(
    screen.getByRole("button", { name: "Save authentication" }),
  ).toBeDisabled();
  fireEvent.change(affiliate, { target: { value: "synthetic-affiliate" } });
  fireEvent.change(screen.getByLabelText("Credential storage"), {
    target: { value: "session_only" },
  });
  fireEvent.click(screen.getByRole("button", { name: "Save authentication" }));
  expect(api).toHaveValue("");
  expect(affiliate).toHaveValue("");
  await screen.findByText(
    "Authentication saved. Provider enablement is unchanged.",
  );
  expect(providerAuthApi.save).toHaveBeenCalledWith({
    provider_id: "dmm",
    method: "affiliate",
    storage: "session_only",
    secret: JSON.stringify({
      api_id: "synthetic-api",
      affiliate_id: "synthetic-affiliate",
    }),
  });
  expect(document.body.textContent).not.toContain("synthetic-api");
  expect(stored.mock.calls.flat().join(" ")).not.toContain("synthetic-api");
  stored.mockRestore();
});
it("does not expose or fetch adult authentication while the workspace is locked", async () => {
  privacy.includeAdult = false;
  show();
  await screen.findByLabelText("Credential");
  expect(
    screen.queryByRole("option", { name: "DMM / FANZA" }),
  ).not.toBeInTheDocument();
  expect(screen.queryByLabelText("DMM API ID")).not.toBeInTheDocument();
  expect(providerAuthApi.status).not.toHaveBeenCalledWith("dmm");
});
it("uses public read-only status for the secondary adult source", async () => {
  show();
  fireEvent.change(screen.getByLabelText("Authentication provider"), {
    target: { value: "wikidata" },
  });
  await screen.findByText(
    "This provider uses public read-only access and requires no credential.",
  );
  expect(screen.queryByLabelText("Credential")).not.toBeInTheDocument();
  expect(providerAuthApi.save).not.toHaveBeenCalled();
});
