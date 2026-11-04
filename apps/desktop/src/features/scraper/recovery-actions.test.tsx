import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import { DesktopError } from "../../api/desktop";
import type { SearchPreview } from "../../api/scraper";
import i18n from "../../i18n";
import { ScraperRecovery } from "./recovery-actions";

const empty: SearchPreview = {
  schema_version: 1,
  run_id: "run-1",
  candidates: [],
  failures: [],
};

beforeEach(async () => {
  await i18n.changeLanguage("en-US");
});

it("only invokes the explicit recovery callback and never submits its parent form", () => {
  const retry = vi.fn();
  const edit = vi.fn();
  const submit = vi.fn();
  render(
    <form onSubmit={submit}>
      <ScraperRecovery search={empty} onRetry={retry} onEdit={edit} />
    </form>,
  );
  expect(retry).not.toHaveBeenCalled();
  expect(edit).not.toHaveBeenCalled();
  fireEvent.click(
    screen.getByRole("button", { name: "Search this item again" }),
  );
  expect(retry).toHaveBeenCalledTimes(1);
  expect(edit).not.toHaveBeenCalled();
  expect(submit).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Edit query" }));
  expect(edit).toHaveBeenCalledTimes(1);
  expect(retry).toHaveBeenCalledTimes(1);
});

it("routes source configuration through the caller while preserving ownership of context", () => {
  const configure = vi.fn();
  render(
    <ScraperRecovery
      search={{
        ...empty,
        failures: [{ provider_id: "tmdb", failure: "credentials_unavailable" }],
      }}
      onConfigure={configure}
    />,
  );
  fireEvent.click(
    screen.getByRole("button", { name: "Configure source and return · TMDB" }),
  );
  expect(configure).toHaveBeenCalledExactlyOnceWith("tmdb");
  expect(
    screen.queryByRole("button", { name: "Search this item again" }),
  ).not.toBeInTheDocument();
});

it("shows no invented cooldown, cancellation or automatic retry for rate limits", () => {
  const retry = vi.fn();
  render(
    <ScraperRecovery
      search={{
        ...empty,
        failures: [{ provider_id: "tmdb", failure: "rate_limited" }],
      }}
      onRetry={retry}
    />,
  );
  expect(screen.getByRole("alert")).toHaveTextContent("Try again later.");
  expect(screen.getAllByRole("button")).toHaveLength(1);
  expect(screen.queryByRole("timer")).not.toBeInTheDocument();
  expect(retry).not.toHaveBeenCalled();
});

it("offers one configuration action per source even when it has several failures", () => {
  render(
    <ScraperRecovery
      search={{
        ...empty,
        failures: [
          { provider_id: "tmdb", failure: "credentials_unavailable" },
          { provider_id: "tmdb", failure: "disabled" },
        ],
      }}
      onConfigure={() => {}}
    />,
  );
  expect(screen.getAllByRole("button")).toHaveLength(1);
});

it("never renders raw errors, correlation IDs or untrusted provider names", () => {
  const error = new DesktopError("request_failed", {
    schema_version: 1,
    code: "unrecognized /private",
    message: "secret-token /home/name",
    correlation_id: "secret-path",
  });
  const view = render(<ScraperRecovery error={error} />);
  expect(screen.getByRole("alert")).toHaveTextContent(
    "The source request failed.",
  );
  expect(view.container.textContent).not.toMatch(/secret|private|\/home/);
  view.rerender(
    <ScraperRecovery
      search={{
        ...empty,
        failures: [
          { provider_id: "/home/name secret-token", failure: "timeout" },
        ],
      }}
    />,
  );
  expect(view.container.textContent).not.toMatch(/secret|\/home/);
  expect(screen.queryByRole("button")).not.toBeInTheDocument();
});

it("disables recovery while a request is running and suppresses desktop-impossible actions", () => {
  const retry = vi.fn();
  const edit = vi.fn();
  const configure = vi.fn();
  const view = render(
    <ScraperRecovery
      busy
      search={{
        ...empty,
        failures: [{ provider_id: "tmdb", failure: "disabled" }],
      }}
      onRetry={retry}
      onEdit={edit}
      onConfigure={configure}
    />,
  );
  for (const button of screen.getAllByRole("button")) {
    expect(button).toBeDisabled();
    fireEvent.click(button);
  }
  expect(retry).not.toHaveBeenCalled();
  expect(edit).not.toHaveBeenCalled();
  expect(configure).not.toHaveBeenCalled();
  view.rerender(
    <ScraperRecovery
      error={new DesktopError("desktop_required")}
      onRetry={retry}
      onEdit={edit}
      onConfigure={configure}
    />,
  );
  expect(screen.getByRole("alert")).toHaveTextContent(
    "Open this in the desktop application.",
  );
  expect(screen.queryByRole("button")).not.toBeInTheDocument();
});

it("renders localized Chinese recovery rather than translation keys", async () => {
  await i18n.changeLanguage("zh-CN");
  render(<ScraperRecovery search={empty} onEdit={() => {}} />);
  expect(screen.getByRole("status")).toHaveTextContent("没有找到匹配结果");
  expect(screen.getByRole("button", { name: "修改查询" })).toBeVisible();
});
