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
function Controls() {
  const { includeAdult, settings, loading, setIncludeAdult, unlock, lock } =
    usePrivacy();
  return (
    <>
      <p>{loading ? "Fixture loading" : "Fixture ready"}</p>
      {settings.entry_visible && <a href="/adult">Fixture adult entry</a>}
      <p>{includeAdult ? "Fixture adult content" : "Fixture content hidden"}</p>
      <button type="button" onClick={() => setIncludeAdult(true)}>
        Fixture show entry
      </button>
      <button type="button" onClick={() => setIncludeAdult(false)}>
        Fixture hide entry
      </button>
      <button type="button" onClick={() => void unlock()}>
        Fixture unlock
      </button>
      <button type="button" onClick={() => void lock()}>
        Fixture lock
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
  const promise = new Promise<T>((r) => {
    resolve = r;
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
  vi.mocked(adultApi.unlock).mockResolvedValue({
    ...closed,
    entry_visible: true,
    unlocked: true,
  });
  vi.mocked(adultApi.lock).mockResolvedValue({
    ...closed,
    entry_visible: true,
  });
});
it("requires a backend session unlock and does not trust the legacy visible preference", async () => {
  localStorage.setItem(ADULT_STORAGE_KEY, "true");
  show();
  await screen.findByText("Fixture ready");
  expect(screen.queryByRole("link")).not.toBeInTheDocument();
  expect(screen.getByText("Fixture content hidden")).toBeInTheDocument();
  fireEvent.click(screen.getByText("Fixture show entry"));
  await screen.findByRole("link", { name: "Fixture adult entry" });
  expect(screen.queryByText("Fixture adult content")).not.toBeInTheDocument();
  fireEvent.click(screen.getByText("Fixture unlock"));
  await screen.findByText("Fixture adult content");
});
it("keeps an enabled entry through lock and restart while clearing private cached data", async () => {
  vi.mocked(adultApi.settings).mockResolvedValue({
    ...closed,
    entry_visible: true,
  });
  const view = show();
  await screen.findByText("Fixture ready");
  fireEvent.click(screen.getByText("Fixture unlock"));
  await screen.findByText("Fixture adult content");
  view.client.setQueryData(["adult", "source", "synthetic"], {
    title: "Synthetic private title",
  });
  fireEvent.click(screen.getByText("Fixture lock"));
  expect(screen.getByText("Fixture content hidden")).toBeInTheDocument();
  expect(
    view.client.getQueryData(["adult", "source", "synthetic"]),
  ).toBeUndefined();
  expect(
    screen.getByRole("link", { name: "Fixture adult entry" }),
  ).toBeInTheDocument();
  await waitFor(() => expect(adultApi.lock).toHaveBeenCalled());
  view.unmount();
  show();
  await screen.findByText("Fixture ready");
  expect(
    screen.getByRole("link", { name: "Fixture adult entry" }),
  ).toBeInTheDocument();
  expect(screen.queryByText("Fixture adult content")).not.toBeInTheDocument();
  fireEvent.click(screen.getByText("Fixture hide entry"));
  expect(screen.queryByRole("link")).not.toBeInTheDocument();
  await waitFor(() =>
    expect(adultApi.saveSettings).toHaveBeenCalledWith({
      entry_visible: false,
      covers_visible: false,
      remember_recent: false,
    }),
  );
});
it("ignores a late unlock response after the user locks", async () => {
  const pending = deferred<AdultSettings>();
  vi.mocked(adultApi.settings).mockResolvedValue({
    ...closed,
    entry_visible: true,
  });
  vi.mocked(adultApi.unlock).mockReturnValue(pending.promise);
  show();
  await screen.findByText("Fixture ready");
  fireEvent.click(screen.getByText("Fixture unlock"));
  fireEvent.click(screen.getByText("Fixture lock"));
  await waitFor(() => expect(adultApi.lock).toHaveBeenCalled());
  await act(async () =>
    pending.resolve({ ...closed, entry_visible: true, unlocked: true }),
  );
  expect(screen.queryByText("Fixture adult content")).not.toBeInTheDocument();
  expect(
    screen.getByRole("link", { name: "Fixture adult entry" }),
  ).toBeInTheDocument();
});
it("ignores a late enable-settings response after hiding the entry", async () => {
  const pending = deferred<AdultSettings>();
  vi.mocked(adultApi.saveSettings)
    .mockReturnValueOnce(pending.promise)
    .mockResolvedValueOnce(closed);
  show();
  await screen.findByText("Fixture ready");
  fireEvent.click(screen.getByText("Fixture show entry"));
  fireEvent.click(screen.getByText("Fixture hide entry"));
  await act(async () =>
    pending.resolve({ ...closed, entry_visible: true, unlocked: true }),
  );
  expect(screen.queryByRole("link")).not.toBeInTheDocument();
  expect(screen.queryByText("Fixture adult content")).not.toBeInTheDocument();
});
