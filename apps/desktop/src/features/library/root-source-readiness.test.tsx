import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, within } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import { providerApi, type ProviderView } from "../../api/providers";
import i18n from "../../i18n";
import { RootSourceReadiness } from "./root-source-readiness";

vi.mock("../../api/providers", () => ({ providerApi: { snapshot: vi.fn(), setEnabled: vi.fn() } }));
const source = (id: string, name: string) => ({
  descriptor: { id, name, fixture: false, media_kinds: ["tv_series"], requires_api_key: false, requires_login: false },
  config: { enabled: true },
}) as ProviderView;

beforeEach(async () => {
  await i18n.changeLanguage("en-US");
  vi.mocked(providerApi.snapshot).mockResolvedValue({
    schema_version: 1, tasks: [], requests: [],
    providers: [source("tmdb", "TMDB"), source("tvmaze", "TVmaze")],
  });
});

it("edits only the selected directory's list and never enables a provider globally", async () => {
  const first = vi.fn();
  const second = vi.fn();
  render(
    <QueryClientProvider client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}>
      <section aria-label="First directory">
        <RootSourceReadiness kind="tv_series" adultAvailable={false} disabled={false} value={["tmdb", "tvmaze"]} onValueChange={first} />
      </section>
      <section aria-label="Second directory">
        <RootSourceReadiness kind="tv_series" adultAvailable={false} disabled={false} value={[]} onValueChange={second} />
      </section>
    </QueryClientProvider>,
  );
  const directory = within(screen.getByRole("region", { name: "First directory" }));
  fireEvent.click(await directory.findByRole("button", { name: "Move TVmaze up" }));
  expect(first).toHaveBeenLastCalledWith(["tvmaze", "tmdb"]);
  fireEvent.click(directory.getByRole("checkbox", { name: /TMDB/ }));
  expect(first).toHaveBeenLastCalledWith(["tvmaze"]);
  expect(second).not.toHaveBeenCalled();
  expect(providerApi.setEnabled).not.toHaveBeenCalled();
  expect(within(screen.getByRole("region", { name: "Second directory" })).getByRole("checkbox", { name: "TMDB" })).not.toBeChecked();
  expect(screen.queryByText("Metadata sources configured")).not.toBeInTheDocument();
});
