import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import type { ReviewQueue } from "../../api/review-queue";
import i18n from "../../i18n";
import { PersistentQueueNavigation } from "./persistent-queue-navigation";
import type { ReviewSession } from "./use-review-queue";

vi.mock("@tanstack/react-router", () => ({
  Link: ({ children }: { children: React.ReactNode }) => (
    <a href="/tools">{children}</a>
  ),
}));
function fixture(count = 32, selected = 0) {
  const queue: ReviewQueue = {
    id: "10000000-0000-4000-8000-000000000001",
    revision: 0,
    current_media_id: `media-${selected}`,
    context: {},
    created_at: "2026-11-04",
    updated_at: "2026-11-04",
    items: Array.from({ length: count }, (_, position) => ({
      media_id: `media-${position}`,
      position,
      state: "pending",
      checkpoint: null,
      preview_id: null,
      needs_recheck: false,
      reason: null,
    })),
  };
  const transition = vi.fn().mockResolvedValue(queue);
  const review = {
    query: { data: queue },
    selected: queue.items[selected],
    transition,
    refresh: vi.fn(),
    isActive: () => true,
  } as unknown as ReviewSession;
  return { queue, review, transition };
}
beforeEach(async () => {
  await i18n.changeLanguage("en-US");
});
it("uses a fixed set of controls for a full queue instead of 32 pending buttons", () => {
  const { review } = fixture();
  render(
    <PersistentQueueNavigation
      review={review}
      busy={false}
      onNavigate={vi.fn()}
    />,
  );
  expect(screen.getAllByRole("button")).toHaveLength(4);
  expect(
    screen.queryByRole("button", { name: /Pending review/ }),
  ).not.toBeInTheDocument();
  expect(screen.getAllByRole("option")).toHaveLength(32);
  const status = screen.getByRole("status");
  expect(status).toHaveAttribute("data-slot", "badge");
  expect(status).toHaveTextContent("Pending review");
  expect(status.tagName).toBe("SPAN");
  expect(status).not.toHaveAttribute("tabindex");
  expect(screen.getByRole("combobox", { name: "Current item" })).toHaveValue(
    "media-0",
  );
  expect(screen.getByRole("button", { name: "Previous" })).toBeDisabled();
});
it("selects the next pending item before navigating", async () => {
  const { review, transition } = fixture(3, 1);
  const navigate = vi.fn();
  render(
    <PersistentQueueNavigation
      review={review}
      busy={false}
      onNavigate={navigate}
    />,
  );
  fireEvent.click(screen.getByRole("button", { name: "Next" }));
  await waitFor(() => expect(navigate).toHaveBeenCalledWith("media-2"));
  expect(transition).toHaveBeenCalledWith("select", "media-2");
});
it("jumps to a deferred item without retrying or confirming it", async () => {
  const { queue, review, transition } = fixture(3);
  queue.items[2].state = "deferred";
  const navigate = vi.fn();
  render(
    <PersistentQueueNavigation
      review={review}
      busy={false}
      onNavigate={navigate}
    />,
  );
  fireEvent.change(screen.getByRole("combobox", { name: "Current item" }), {
    target: { value: "media-2" },
  });
  await waitFor(() => expect(navigate).toHaveBeenCalledWith("media-2"));
  expect(transition).not.toHaveBeenCalled();
});
it("does not navigate after a conflicting selection or while a write is in progress", async () => {
  const { review, transition } = fixture(3);
  transition.mockRejectedValue(new Error("conflict"));
  const navigate = vi.fn();
  const view = render(
    <PersistentQueueNavigation
      review={review}
      busy={false}
      onNavigate={navigate}
    />,
  );
  fireEvent.click(screen.getByRole("button", { name: "Next" }));
  await waitFor(() => expect(transition).toHaveBeenCalledOnce());
  expect(navigate).not.toHaveBeenCalled();
  view.rerender(
    <PersistentQueueNavigation review={review} busy onNavigate={navigate} />,
  );
  expect(screen.getByRole("combobox", { name: "Current item" })).toBeDisabled();
  expect(screen.getByRole("button", { name: "Next" })).toBeDisabled();
});
