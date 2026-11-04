import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import { nfoApi, type NfoDocument } from "../../api/nfo";
import i18n from "../../i18n";
import { NfoPage } from "./page";

const events = vi.hoisted(() => ({ opened: (() => {}) as () => void }));
vi.mock("@tauri-apps/api/core", () => ({ isTauri: () => true }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async (_name: string, callback: () => void) => {
    events.opened = callback;
    return () => {};
  }),
}));
vi.mock("../../api/nfo", () => ({
  nfoApi: { takeOpened: vi.fn(), open: vi.fn() },
}));

const document: NfoDocument = {
  name: "sample.nfo",
  format: "xml",
  source: "<movie><title>Atlas</title></movie>",
  metadata: { fields: { title: "Atlas", overview: "<img src=x onerror=alert(1)>" }, external_ids: [] },
};

beforeEach(async () => {
  vi.mocked(nfoApi.takeOpened).mockReset().mockResolvedValue(null);
  vi.mocked(nfoApi.open).mockReset();
  await i18n.changeLanguage("en-US");
});

it("previews escaped text and keeps the document when file selection is cancelled", async () => {
  vi.mocked(nfoApi.takeOpened).mockResolvedValueOnce(document);
  vi.mocked(nfoApi.open).mockResolvedValueOnce(null);
  const { container } = render(<NfoPage />);
  await screen.findByRole("heading", { name: "Atlas" });
  expect(container.querySelector("img,video,audio,iframe")).toBeNull();
  expect(screen.getByText("<img src=x onerror=alert(1)>")).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Open NFO" }));
  await waitFor(() => expect(nfoApi.open).toHaveBeenCalledWith("en-US"));
  expect(screen.getByRole("heading", { name: "Atlas" })).toBeInTheDocument();
});

it("serializes startup and opened events so an empty replay cannot discard the preview", async () => {
  let finish!: (value: NfoDocument) => void;
  vi.mocked(nfoApi.takeOpened).mockImplementationOnce(() => new Promise((resolve) => { finish = resolve; }));
  render(<NfoPage />);
  await waitFor(() => expect(nfoApi.takeOpened).toHaveBeenCalledTimes(1));
  act(() => events.opened());
  expect(nfoApi.takeOpened).toHaveBeenCalledTimes(1);
  await act(async () => finish(document));
  await waitFor(() => expect(nfoApi.takeOpened).toHaveBeenCalledTimes(2));
  expect(screen.getByRole("heading", { name: "Atlas" })).toBeInTheDocument();
});
