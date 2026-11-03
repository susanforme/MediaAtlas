import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { I18nextProvider } from "react-i18next";
import { beforeEach, expect, it, vi } from "vitest";
import { DesktopError } from "../../api/desktop";
import { type AuthStatus, providerAuthApi } from "../../api/provider-auth";
import i18n from "../../i18n";
import { ProviderAuthSettings } from "./provider-auth";

vi.mock("../../api/provider-auth", async (original) => ({
  ...(await original<typeof import("../../api/provider-auth")>()),
  providerAuthApi: { status: vi.fn(), save: vi.fn(), clear: vi.fn() },
}));
const status: AuthStatus = {
  provider_id: "tmdb",
  supported_methods: ["bearer", "api_key"],
  method: "bearer",
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
  await i18n.changeLanguage("en-US");
  vi.mocked(providerAuthApi.status).mockImplementation(async (provider) =>
    provider === "tmdb"
      ? status
      : {
          ...status,
          provider_id: provider,
          supported_methods: ["none"],
          method: "none",
          configured: false,
          status: "not_required",
        },
  );
  vi.mocked(providerAuthApi.save).mockImplementation(async (input) => ({
    ...status,
    method: input.method,
    storage: input.storage,
    configured: true,
    status: "configured",
  }));
  vi.mocked(providerAuthApi.clear).mockResolvedValue(status);
});
it("defaults to OS storage, clears the password before saving and never enables providers", async () => {
  const stored = vi.spyOn(Storage.prototype, "setItem");
  show();
  const input = await screen.findByLabelText("Credential");
  expect(input).toHaveAttribute("type", "password");
  expect(screen.getByLabelText("Credential storage")).toHaveValue("system");
  fireEvent.change(input, { target: { value: "synthetic-bearer-token" } });
  fireEvent.click(screen.getByRole("button", { name: "Save authentication" }));
  expect(input).toHaveValue("");
  await screen.findByText(
    "Authentication saved. Provider enablement is unchanged.",
  );
  expect(providerAuthApi.save).toHaveBeenCalledWith({
    provider_id: "tmdb",
    method: "bearer",
    storage: "system",
    secret: "synthetic-bearer-token",
  });
  expect(
    screen.queryByDisplayValue("synthetic-bearer-token"),
  ).not.toBeInTheDocument();
  expect(screen.queryByText("synthetic-bearer-token")).not.toBeInTheDocument();
  expect(
    stored.mock.calls.every(
      (call) =>
        !call.some((value) => String(value).includes("synthetic-bearer-token")),
    ),
  ).toBe(true);
  stored.mockRestore();
});
it("requires an explicit session-only selection and supports the TMDB v3 key method", async () => {
  show();
  await screen.findByLabelText("Credential");
  fireEvent.change(screen.getByLabelText("Authentication method"), {
    target: { value: "api_key" },
  });
  expect(screen.getByLabelText("Authentication method")).toHaveValue("api_key");
  fireEvent.change(screen.getByLabelText("Credential storage"), {
    target: { value: "session_only" },
  });
  expect(screen.getByLabelText("Credential storage")).toHaveValue(
    "session_only",
  );
  expect(
    screen.getByRole("option", { name: "This session only (cleared on exit)" }),
  ).toBeInTheDocument();
  fireEvent.change(screen.getByLabelText("Credential"), {
    target: { value: "synthetic-v3-key" },
  });
  fireEvent.click(screen.getByRole("button", { name: "Save authentication" }));
  await screen.findByText(
    "Authentication saved. Provider enablement is unchanged.",
  );
  expect(providerAuthApi.save).toHaveBeenCalledWith({
    provider_id: "tmdb",
    method: "api_key",
    storage: "session_only",
    secret: "synthetic-v3-key",
  });
});
it("reports system-store failures without saving or silently falling back", async () => {
  vi.mocked(providerAuthApi.save).mockRejectedValue(
    new DesktopError("request_failed", {
      schema_version: 1,
      code: "credentials_unavailable",
      message: "unused",
      correlation_id: "test",
    }),
  );
  show();
  await screen.findByLabelText("Credential");
  fireEvent.change(screen.getByLabelText("Credential"), {
    target: { value: "synthetic-token" },
  });
  fireEvent.click(screen.getByRole("button", { name: "Save authentication" }));
  await screen.findByText(
    "Authentication was not saved. Check the error and explicitly choose session-only storage if the system store is unavailable.",
  );
  expect(screen.getByLabelText("Credential")).toHaveValue("");
  expect(screen.getByLabelText("Credential storage")).toHaveValue("system");
  expect(
    screen.queryByText(
      "Authentication saved. Provider enablement is unchanged.",
    ),
  ).not.toBeInTheDocument();
  expect(providerAuthApi.save).toHaveBeenCalledTimes(1);
});
it("resets inputs on provider and method changes, with public providers showing no secret form", async () => {
  show();
  await screen.findByLabelText("Credential");
  fireEvent.change(screen.getByLabelText("Credential"), {
    target: { value: "synthetic-unsaved-token" },
  });
  fireEvent.change(screen.getByLabelText("Authentication method"), {
    target: { value: "api_key" },
  });
  expect(screen.getByLabelText("Credential")).toHaveValue("");
  fireEvent.change(screen.getByLabelText("Credential"), {
    target: { value: "another-unsaved-key" },
  });
  fireEvent.change(screen.getByLabelText("Authentication provider"), {
    target: { value: "anilist" },
  });
  await screen.findByText(
    "This provider uses public read-only access and requires no credential.",
  );
  expect(screen.queryByLabelText("Credential")).not.toBeInTheDocument();
  fireEvent.change(screen.getByLabelText("Authentication provider"), {
    target: { value: "wikipedia.html" },
  });
  await waitFor(() =>
    expect(providerAuthApi.status).toHaveBeenCalledWith("wikipedia.html"),
  );
  expect(
    screen.queryByRole("button", { name: "Save authentication" }),
  ).not.toBeInTheDocument();
  fireEvent.change(screen.getByLabelText("Authentication provider"), {
    target: { value: "tmdb" },
  });
  await screen.findByLabelText("Credential");
  expect(screen.getByLabelText("Credential")).toHaveValue("");
  expect(providerAuthApi.save).not.toHaveBeenCalled();
});
it("ignores late save results after changing providers", async () => {
  let resolveSave: ((value: AuthStatus) => void) | undefined;
  vi.mocked(providerAuthApi.save).mockImplementation(
    () =>
      new Promise((resolve) => {
        resolveSave = resolve;
      }),
  );
  show();
  await screen.findByLabelText("Credential");
  fireEvent.change(screen.getByLabelText("Credential"), {
    target: { value: "synthetic-token" },
  });
  fireEvent.click(screen.getByRole("button", { name: "Save authentication" }));
  fireEvent.change(screen.getByLabelText("Authentication provider"), {
    target: { value: "anilist" },
  });
  await screen.findByText(
    "This provider uses public read-only access and requires no credential.",
  );
  resolveSave?.({ ...status, configured: true });
  await waitFor(() =>
    expect(
      screen.queryByText(
        "Authentication saved. Provider enablement is unchanged.",
      ),
    ).not.toBeInTheDocument(),
  );
  expect(screen.queryByLabelText("Credential")).not.toBeInTheDocument();
});
it("deletes authentication and clears input without echoing the saved value", async () => {
  vi.mocked(providerAuthApi.status).mockResolvedValue({
    ...status,
    configured: true,
  });
  show();
  await screen.findByText("Credential configured");
  fireEvent.change(screen.getByLabelText("Credential"), {
    target: { value: "unsaved-replacement" },
  });
  fireEvent.click(
    screen.getByRole("button", { name: "Delete saved authentication" }),
  );
  await screen.findByText("Saved authentication deleted.");
  expect(providerAuthApi.clear).toHaveBeenCalledWith("tmdb");
  expect(screen.getByLabelText("Credential")).toHaveValue("");
  expect(screen.getByText("No credential configured")).toBeInTheDocument();
});
it("keeps system storage as the form default even if a session credential already exists", async () => {
  vi.mocked(providerAuthApi.status).mockResolvedValue({
    ...status,
    configured: true,
    storage: "session_only",
  });
  show();
  await screen.findByText("Credential configured");
  expect(screen.getByLabelText("Credential storage")).toHaveValue("system");
  expect(
    screen.getByText("Current storage: This session only (cleared on exit)"),
  ).toBeInTheDocument();
});
it("supports the full save and delete interaction in Simplified Chinese", async () => {
  await i18n.changeLanguage("zh-CN");
  show();
  await screen.findByLabelText("凭据");
  expect(screen.getByRole("heading", { name: "来源认证" })).toBeInTheDocument();
  fireEvent.change(screen.getByLabelText("凭据存储方式"), {
    target: { value: "session_only" },
  });
  fireEvent.change(screen.getByLabelText("凭据"), {
    target: { value: "synthetic-zh-token" },
  });
  fireEvent.click(screen.getByRole("button", { name: "保存认证" }));
  await screen.findByText("认证已保存，来源启用状态保持不变。");
  expect(screen.getByLabelText("凭据")).toHaveValue("");
  fireEvent.click(screen.getByRole("button", { name: "删除已保存认证" }));
  await screen.findByText("已删除保存的认证。");
});
it("does not report saved when a response says the credential is unavailable", async () => {
  vi.mocked(providerAuthApi.save).mockResolvedValue({
    ...status,
    configured: false,
    status: "credentials_unavailable",
  });
  show();
  await screen.findByLabelText("Credential");
  fireEvent.change(screen.getByLabelText("Credential"), {
    target: { value: "synthetic-token" },
  });
  fireEvent.click(screen.getByRole("button", { name: "Save authentication" }));
  await screen.findByText(
    "Authentication was not saved. Check the error and explicitly choose session-only storage if the system store is unavailable.",
  );
  expect(screen.getByLabelText("Credential")).toHaveValue("");
  expect(
    screen.queryByText(
      "Authentication saved. Provider enablement is unchanged.",
    ),
  ).not.toBeInTheDocument();
});
it("shows saved authentication with pending cleanup and retries cleanup through status refresh", async () => {
  vi.mocked(providerAuthApi.status)
    .mockResolvedValueOnce({
      ...status,
      configured: true,
      status: "cleanup_pending",
    })
    .mockResolvedValue({ ...status, configured: true, status: "configured" });
  show();
  await screen.findByText(
    "Authentication saved; old credentials are awaiting secure cleanup. Refresh the status to retry.",
  );
  expect(screen.getByText("Credential configured")).toBeInTheDocument();
  expect(
    screen.queryByText("Saved authentication deleted."),
  ).not.toBeInTheDocument();
  await i18n.changeLanguage("zh-CN");
  await screen.findByText("认证已保存；旧凭据等待安全清理，可刷新状态重试。");
  fireEvent.click(screen.getByRole("button", { name: "刷新认证状态" }));
  await waitFor(() => expect(providerAuthApi.status).toHaveBeenCalledTimes(2));
  await waitFor(() =>
    expect(
      screen.queryByText("认证已保存；旧凭据等待安全清理，可刷新状态重试。"),
    ).not.toBeInTheDocument(),
  );
  expect(providerAuthApi.save).not.toHaveBeenCalled();
  expect(providerAuthApi.clear).not.toHaveBeenCalled();
});
it("distinguishes disabled authentication from completed physical credential cleanup", async () => {
  vi.mocked(providerAuthApi.status).mockResolvedValue({
    ...status,
    configured: true,
  });
  vi.mocked(providerAuthApi.clear).mockResolvedValue({
    ...status,
    configured: false,
    status: "cleanup_pending",
  });
  show();
  await screen.findByText("Credential configured");
  fireEvent.click(
    screen.getByRole("button", { name: "Delete saved authentication" }),
  );
  await screen.findByText(
    "Authentication is disabled; old credentials are awaiting secure cleanup. Refresh the status to retry.",
  );
  expect(
    screen.queryByText("Saved authentication deleted."),
  ).not.toBeInTheDocument();
  expect(
    screen.queryByText(
      "Authentication saved; old credentials are awaiting secure cleanup. Refresh the status to retry.",
    ),
  ).not.toBeInTheDocument();
  expect(screen.getByText("No credential configured")).toBeInTheDocument();
});
