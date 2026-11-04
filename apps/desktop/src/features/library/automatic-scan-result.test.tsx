import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, within } from "@testing-library/react";
import type { ReactNode } from "react";
import { beforeEach, expect, it, vi } from "vitest";
import { libraryApi, type ScanTask } from "../../api/library";
import i18n from "../../i18n";
import { AutomaticScanResult } from "./automatic-scan-result";
import { LibraryActivity } from "./library-activity";
import { useLibraryActivity } from "./use-library-activity";

vi.mock("@tanstack/react-router", () => ({
  Link: ({
    to,
    search,
    children,
  }: {
    to: string;
    search?: unknown;
    children: ReactNode;
  }) => (
    <a href={to} data-search={JSON.stringify(search)}>
      {children}
    </a>
  ),
}));
vi.mock("../../api/library", async (original) => ({
  ...(await original<typeof import("../../api/library")>()),
  libraryApi: { tasks: vi.fn() },
}));

const task: ScanTask = {
  id: "refresh",
  root_id: "folder",
  mode: "automatic",
  phase: "complete",
  state: "waiting_for_user",
  visited_files: 3,
  skipped_files: 0,
  candidate_count: 3,
  imported_count: 3,
  matched_count: 0,
  review_count: 3,
  unconfigured_count: 3,
  metadata_failed_count: 0,
  warnings: 0,
  error_code: null,
  created_at: "2026-11-04T10:00:00Z",
  updated_at: "2026-11-04T10:00:01Z",
};

beforeEach(async () => {
  await i18n.changeLanguage("zh-CN");
});

function Activity() {
  return <LibraryActivity activity={useLibraryActivity()} />;
}

it("shows missing source setup instead of scrape success after local indexing", () => {
  render(<AutomaticScanResult task={task} />);
  expect(screen.getByRole("status")).toHaveTextContent("等待配置刮削来源");
  expect(screen.queryByText("媒体库已更新")).not.toBeInTheDocument();
  expect(
    screen.queryByRole("link", { name: "处理未匹配媒体" }),
  ).not.toBeInTheDocument();
  expect(screen.getByRole("link", { name: "查看媒体库" })).toHaveAttribute(
    "href",
    "/library",
  );
  const setup = screen.getByRole("link", { name: "配置来源并继续" });
  expect(setup).toHaveAttribute("href", "/settings");
  expect(setup).toHaveAttribute(
    "data-search",
    JSON.stringify({ from: "library", root: "folder" }),
  );
  const unconfigured = screen.getByText("等待来源配置").parentElement;
  expect(unconfigured).not.toBeNull();
  expect(
    within(unconfigured as HTMLElement).getByText("3"),
  ).toBeInTheDocument();
});

it("separates genuine ambiguous matches from items that have never been scraped", () => {
  render(<AutomaticScanResult task={{ ...task, unconfigured_count: 2 }} />);
  const review = screen.getByText("待处理资料").parentElement;
  expect(within(review as HTMLElement).getByText("1")).toBeInTheDocument();
  expect(screen.getByRole("link", { name: "处理未匹配媒体" })).toHaveAttribute(
    "data-search",
    JSON.stringify({ root: "folder", health: "metadata_needs_review" }),
  );
});

it("preserves the explicit adult type when opening source configuration", () => {
  render(<AutomaticScanResult task={task} adult />);
  expect(screen.getByRole("link", { name: "配置来源并继续" })).toHaveAttribute(
    "data-search",
    JSON.stringify({ from: "library", root: "folder", kind: "adult_video" }),
  );
});

it("describes a successful scan with no newly matched metadata as local scanning", () => {
  render(
    <AutomaticScanResult
      task={{
        ...task,
        state: "succeeded",
        review_count: 0,
        unconfigured_count: 0,
      }}
    />,
  );
  expect(screen.getByRole("status")).toHaveTextContent("本地目录扫描完成");
  expect(screen.queryByText("目录更新及资料匹配完成")).not.toBeInTheDocument();
});

it("keeps a source setup prompt visible in the library after the worker stops", async () => {
  vi.mocked(libraryApi.tasks).mockResolvedValue([task]);
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  const view = render(
    <QueryClientProvider client={client}>
      <Activity />
    </QueryClientProvider>,
  );
  expect(await screen.findByText("等待配置刮削来源")).toBeInTheDocument();
  expect(screen.getByText("3 项等待来源配置")).toBeInTheDocument();
  expect(screen.queryByText("待核对 3 项")).not.toBeInTheDocument();
  expect(screen.getByRole("link", { name: "配置来源并继续" })).toHaveAttribute(
    "data-search",
    JSON.stringify({ from: "library", root: "folder" }),
  );
  view.unmount();
  client.clear();
});

it("opens only the actionable review and failure sets in the original folder", async () => {
  vi.mocked(libraryApi.tasks).mockResolvedValue([
    {
      ...task,
      review_count: 3,
      unconfigured_count: 1,
      metadata_failed_count: 2,
    },
  ]);
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  render(
    <QueryClientProvider client={client}>
      <Activity />
    </QueryClientProvider>,
  );
  const review = await screen.findByRole("link", { name: "处理未识别媒体" });
  expect(JSON.parse(review.getAttribute("data-search") ?? "{}")).toEqual({
    root: "folder",
    health: "metadata_needs_review",
  });
  const failed = screen.getByRole("link", { name: "查看资料获取失败的媒体" });
  expect(JSON.parse(failed.getAttribute("data-search") ?? "{}")).toEqual({
    root: "folder",
    health: "metadata_failed",
  });
  expect(
    screen.getByRole("link", { name: "配置来源并继续" }),
  ).toBeInTheDocument();
});
