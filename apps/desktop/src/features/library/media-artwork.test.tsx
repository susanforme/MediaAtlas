import { fireEvent, render, screen } from "@testing-library/react";
import { I18nextProvider } from "react-i18next";
import { expect, it } from "vitest";
import i18n from "../../i18n";
import { MediaArtwork } from "./media-artwork";

it("replaces broken cached artwork and permits a new cache URI to load", () => {
  const show = (uri: string) => (
    <I18nextProvider i18n={i18n}>
      <MediaArtwork uri={uri} title="Synthetic film" />
    </I18nextProvider>
  );
  const view = render(show("data:image/png;base64,YQ=="));
  fireEvent.error(screen.getByRole("img", { name: "Synthetic film" }));
  expect(screen.queryByRole("img")).not.toBeInTheDocument();
  expect(screen.getByText(i18n.t("noPoster"))).toBeInTheDocument();
  view.rerender(show("data:image/png;base64,Yg=="));
  expect(
    screen.getByRole("img", { name: "Synthetic film" }),
  ).toBeInTheDocument();
  view.rerender(show("https://example.invalid/remote.jpg"));
  expect(screen.queryByRole("img")).not.toBeInTheDocument();
});
