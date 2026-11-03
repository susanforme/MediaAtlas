import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, expect, it } from "vitest";
import { ADULT_STORAGE_KEY, PrivacyProvider, usePrivacy } from "./context";

function Controls() {
  const { includeAdult, setIncludeAdult } = usePrivacy();
  return (
    <button type="button" onClick={() => setIncludeAdult(!includeAdult)}>
      {includeAdult ? "Fixture hide adult" : "Fixture show adult"}
    </button>
  );
}
beforeEach(() => localStorage.clear());
it("hides adult media by default and clears private query data when disabled", () => {
  const client = new QueryClient();
  render(
    <QueryClientProvider client={client}>
      <PrivacyProvider>
        <Controls />
      </PrivacyProvider>
    </QueryClientProvider>,
  );
  fireEvent.click(screen.getByRole("button", { name: "Fixture show adult" }));
  expect(localStorage.getItem(ADULT_STORAGE_KEY)).toBe("true");
  client.setQueryData(["detail", "fixture-adult", true], {
    title: "Fixture adult title",
  });
  fireEvent.click(screen.getByRole("button", { name: "Fixture hide adult" }));
  expect(
    client.getQueryData(["detail", "fixture-adult", true]),
  ).toBeUndefined();
  expect(localStorage.getItem(ADULT_STORAGE_KEY)).toBe("false");
});
