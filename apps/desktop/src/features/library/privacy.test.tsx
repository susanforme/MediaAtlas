import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import {
  type AdultSettings,
  adultPrivacyApi as adultApi,
} from "../../api/adult-privacy";
import { ADULT_STORAGE_KEY, PrivacyProvider, usePrivacy } from "./context";

vi.mock("../../api/adult-privacy", async (original) => ({
  ...(await original<typeof import("../../api/adult-privacy")>()),
  adultPrivacyApi: {
    settings: vi.fn(),
    saveSettings: vi.fn(),
    unlock: vi.fn(),
    lock: vi.fn(),
  },
}));
const closed: AdultSettings = {
  entry_visible: false,
  covers_visible: false,
  remember_recent: false,
  unlocked: false,
};
const enabled: AdultSettings = {
  ...closed,
  entry_visible: true,
  covers_visible: true,
  unlocked: true,
};
function Controls() {
  const { includeAdult, settings, loading, setIncludeAdult, error } =
    usePrivacy();
  return (
    <>
      <p>{loading ? "Fixture loading" : "Fixture ready"}</p>
      {settings.entry_visible && <span>Fixture category</span>}
      <p>{includeAdult ? "Fixture content" : "Fixture hidden"}</p>
      {error !== undefined && <p>Fixture unavailable</p>}
      <button type="button" onClick={() => setIncludeAdult(true)}>
        Fixture enable
      </button>
      <button type="button" onClick={() => setIncludeAdult(false)}>
        Fixture disable
      </button>
    </>
  );
}
function show(client = new QueryClient()) {
  return {
    ...render(
      <QueryClientProvider client={client}>
        <PrivacyProvider>
          <Controls />
        </PrivacyProvider>
      </QueryClientProvider>,
    ),
    client,
  };
}
function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}
beforeEach(() => {
  vi.clearAllMocks();
  localStorage.clear();
  vi.mocked(adultApi.settings).mockResolvedValue(closed);
  vi.mocked(adultApi.saveSettings).mockImplementation(async (value) => ({
    ...value,
    unlocked: false,
  }));
  vi.mocked(adultApi.unlock).mockResolvedValue(enabled);
  vi.mocked(adultApi.lock).mockResolvedValue(closed);
});

it("uses the saved content setting and grants the backend session without a second UI step", async () => {
  localStorage.setItem(ADULT_STORAGE_KEY, "true");
  show();
  await screen.findByText("Fixture ready");
  expect(screen.queryByText("Fixture category")).not.toBeInTheDocument();
  expect(adultApi.unlock).not.toHaveBeenCalled();
  fireEvent.click(screen.getByText("Fixture enable"));
  await screen.findByText("Fixture content");
  expect(adultApi.saveSettings).toHaveBeenCalledWith({
    entry_visible: true,
    covers_visible: true,
    remember_recent: false,
  });
  expect(adultApi.unlock).toHaveBeenCalledTimes(1);
});

it("restores an enabled type automatically and clears private caches immediately when disabled", async () => {
  vi.mocked(adultApi.settings).mockResolvedValue({
    ...enabled,
    unlocked: false,
  });
  const view = show();
  await screen.findByText("Fixture content");
  view.client.setQueryData(["adult", "source", "fixture"], {
    title: "Fixture private metadata",
  });
  fireEvent.click(screen.getByText("Fixture disable"));
  expect(screen.getByText("Fixture hidden")).toBeInTheDocument();
  expect(screen.queryByText("Fixture category")).not.toBeInTheDocument();
  expect(
    view.client.getQueryData(["adult", "source", "fixture"]),
  ).toBeUndefined();
  await waitFor(() =>
    expect(adultApi.saveSettings).toHaveBeenCalledWith({
      entry_visible: false,
      covers_visible: false,
      remember_recent: false,
    }),
  );
});

it("ignores a late automatic grant after the type is disabled", async () => {
  const pending = deferred<AdultSettings>();
  vi.mocked(adultApi.settings).mockResolvedValue({
    ...enabled,
    unlocked: false,
  });
  vi.mocked(adultApi.unlock).mockReturnValue(pending.promise);
  show();
  await waitFor(() => expect(adultApi.unlock).toHaveBeenCalled());
  expect(screen.getByText("Fixture hidden")).toBeInTheDocument();
  fireEvent.click(screen.getByText("Fixture disable"));
  await screen.findByText("Fixture ready");
  await act(async () => pending.resolve(enabled));
  expect(screen.queryByText("Fixture content")).not.toBeInTheDocument();
  expect(screen.queryByText("Fixture category")).not.toBeInTheDocument();
});

it("does not grant from a stale enable-settings response", async () => {
  const pending = deferred<AdultSettings>();
  vi.mocked(adultApi.saveSettings)
    .mockReturnValueOnce(pending.promise)
    .mockResolvedValueOnce(closed);
  show();
  await screen.findByText("Fixture ready");
  fireEvent.click(screen.getByText("Fixture enable"));
  await waitFor(() => expect(adultApi.saveSettings).toHaveBeenCalledTimes(1));
  fireEvent.click(screen.getByText("Fixture disable"));
  expect(adultApi.saveSettings).toHaveBeenCalledTimes(1);
  await act(async () => pending.resolve({ ...enabled, unlocked: false }));
  await waitFor(() => expect(adultApi.saveSettings).toHaveBeenCalledTimes(2));
  expect(vi.mocked(adultApi.saveSettings).mock.calls[1][0].entry_visible).toBe(
    false,
  );
  expect(adultApi.unlock).not.toHaveBeenCalled();
  expect(screen.queryByText("Fixture content")).not.toBeInTheDocument();
});

it("keeps backend data unavailable on authorization failure", async () => {
  vi.mocked(adultApi.settings).mockResolvedValue({
    ...enabled,
    unlocked: false,
  });
  vi.mocked(adultApi.unlock).mockRejectedValue(
    new Error("fixture unavailable"),
  );
  show();
  await screen.findByText("Fixture ready");
  expect(screen.getByText("Fixture unavailable")).toBeInTheDocument();
  expect(screen.queryByText("Fixture content")).not.toBeInTheDocument();
});

it("migrates enabled legacy display preferences to the ordinary category presentation", async () => {
  vi.mocked(adultApi.settings).mockResolvedValue({
    ...closed,
    entry_visible: true,
    remember_recent: true,
  });
  show();
  await screen.findByText("Fixture content");
  expect(adultApi.saveSettings).toHaveBeenCalledWith({
    entry_visible: true,
    covers_visible: true,
    remember_recent: false,
  });
  expect(adultApi.unlock).toHaveBeenCalledTimes(1);
});

it("does not grant from a settings response after the provider unmounts", async () => {
  const pending = deferred<AdultSettings>();
  vi.mocked(adultApi.settings).mockReturnValue(pending.promise);
  const view = show();
  view.unmount();
  await act(async () => pending.resolve({ ...enabled, unlocked: false }));
  expect(adultApi.unlock).not.toHaveBeenCalled();
});
