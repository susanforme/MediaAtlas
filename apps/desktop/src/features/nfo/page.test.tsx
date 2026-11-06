import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { StrictMode } from "react";
import { beforeEach, expect, it, vi } from "vitest";
import { nfoApi, type NfoDocument } from "../../api/nfo";
import i18n from "../../i18n";
import { NfoPage, NfoStandalone } from "./page";

vi.mock("@tauri-apps/api/core", () => ({ isTauri: () => true }));
vi.mock("@tauri-apps/api/window", () => ({ getCurrentWindow: () => ({ label: "nfo-11111111-1111-4111-8111-111111111111" }) }));
vi.mock("../../api/nfo", () => ({ nfoApi: { takeOpened: vi.fn() } }));

const document: NfoDocument = {
  name: "sample.nfo", format: "xml", source: "<movie><title>Atlas</title></movie>",
  poster_uri: null, backdrop_uri: null,
  metadata: { fields: { title: "Atlas", overview: "<img src=x onerror=alert(1)>" }, external_ids: [] },
};

beforeEach(async () => {
  vi.spyOn(navigator, "platform", "get").mockReturnValue("MacIntel");
  vi.mocked(nfoApi.takeOpened).mockReset().mockResolvedValue(null);
  await i18n.changeLanguage("en-US");
});

it("shows core content with a dedicated draggable titlebar and no application shell", async () => {
  vi.mocked(nfoApi.takeOpened).mockResolvedValueOnce(document);
  const { container } = render(<NfoStandalone />);
  await screen.findByRole("heading", { name: "Atlas" });
  expect(container.querySelector('[data-slot="media-detail"]')).not.toBeNull();
  const titlebar = container.querySelector('[data-slot="window-titlebar"]');
  expect(titlebar).not.toBeNull();
  expect(titlebar).toHaveAttribute("data-tauri-drag-region", "deep");
  expect(titlebar).toHaveAttribute("data-window-platform", "macos");
  expect(container.querySelector('[data-slot="app-shell"],nav,.app-toolbar')).toBeNull();
  expect(screen.queryByRole("button", { name: "Open NFO" })).not.toBeInTheDocument();
  expect(container.querySelector("img,video,audio,iframe")).toBeNull();
  expect(screen.getAllByText("<img src=x onerror=alert(1)>").length).toBeGreaterThan(0);
});

it("retains the single selected document across StrictMode effect replay", async () => {
  let finish!: (value: NfoDocument) => void;
  vi.mocked(nfoApi.takeOpened).mockImplementationOnce(() => new Promise((resolve) => { finish = resolve; }));
  render(<StrictMode><NfoPage /></StrictMode>);
  await waitFor(() => expect(nfoApi.takeOpened).toHaveBeenCalledTimes(1));
  await act(async () => finish(document));
  expect(await screen.findByRole("heading", { name: "Atlas" })).toBeInTheDocument();
});

it("shares cast and metadata tabs and renders ratings and collections without JSON", async () => {
  vi.mocked(nfoApi.takeOpened).mockResolvedValueOnce({
    ...document,
    metadata: { external_ids: [], fields: {
      title: "Atlas", cast: [{ name: "Actor A", role: "Lead", thumb: "https://example.test/actor.jpg" }],
      people: ["Actor A"], ratings: [{ name: "tmdb", value: 8.4, max: 10, votes: 123 }],
      collection: { name: "Atlas collection", overview: "Collection story" },
      nfo_artwork: [{ kind: "poster", url: "https://example.test/poster.jpg" }],
    } },
  });
  const { container } = render(<NfoPage />);
  await screen.findByRole("heading", { name: "Atlas" });
  expect(screen.getByRole("tab", { name: "Metadata and sources" })).toHaveAttribute("aria-selected", "true");
  expect(screen.getByText("8.4 / 10")).toBeInTheDocument();
  expect(screen.getByText("123 votes")).toBeInTheDocument();
  expect(screen.getByText("Atlas collection")).toBeInTheDocument();
  expect(screen.getByText("Collection story")).toBeInTheDocument();
  expect(container.textContent).not.toContain('"name":');
  expect(container.textContent).not.toContain("https://example.test");
  fireEvent.keyDown(screen.getByRole("tab", { name: "Metadata and sources" }), { key: "ArrowLeft" });
  expect(screen.getByRole("tab", { name: "Cast and crew" })).toHaveAttribute("aria-selected", "true");
  expect(screen.getByRole("tabpanel")).toHaveTextContent("Actor A (Lead)");
  expect(screen.queryByRole("tab", { name: "Local files" })).not.toBeInTheDocument();
  expect(screen.queryByRole("tab", { name: "Seasons and episodes" })).not.toBeInTheDocument();
  expect(screen.queryByText("Metadata and media status")).not.toBeInTheDocument();
});

it("uses only validated image data for the shared hero, poster and artwork tab", async () => {
  const poster = "data:image/png;base64,cG9zdGVy";
  const backdrop = "data:image/png;base64,YmFja2Ryb3A=";
  vi.mocked(nfoApi.takeOpened).mockResolvedValueOnce({ ...document, poster_uri: poster, backdrop_uri: backdrop });
  const { container } = render(<NfoPage />);
  await screen.findByRole("heading", { name: "Atlas" });
  expect(container.querySelector('[data-slot="media-backdrop"] img')).toHaveAttribute("src", backdrop);
  expect(container.querySelector("aside img")).toHaveAttribute("src", poster);
  fireEvent.click(screen.getByRole("tab", { name: "Artwork" }));
  expect(screen.getByRole("tabpanel").querySelectorAll("img")).toHaveLength(2);
  fireEvent.error(container.querySelector("aside img")!);
  expect(container.querySelector("aside img")).toBeNull();
});

it("omits missing regions and retries a read failure for the same selected file", async () => {
  vi.mocked(nfoApi.takeOpened).mockRejectedValueOnce({ code: "local_file_unreadable" })
    .mockResolvedValueOnce({ ...document, metadata: { fields: {}, external_ids: [] } });
  const { container } = render(<NfoPage />);
  fireEvent.click(await screen.findByRole("button", { name: "Check again" }));
  await screen.findByRole("heading", { level: 1, name: "sample.nfo" });
  expect(screen.getAllByText("sample.nfo")).toHaveLength(1);
  expect(container.querySelector("aside,img,a[href]")).toBeNull();
  expect(screen.queryByText("Director")).not.toBeInTheDocument();
});
