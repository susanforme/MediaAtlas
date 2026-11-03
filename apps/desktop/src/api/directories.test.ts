import { isTauri } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { beforeEach, expect, it, vi } from "vitest";
import { pickDirectory } from "./directories";

vi.mock("@tauri-apps/api/core", () => ({ isTauri: vi.fn() }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(isTauri).mockReturnValue(true);
});

it("opens a single non-recursive native directory selection and preserves the selected path", async () => {
  vi.mocked(open).mockResolvedValue("/media/电影");
  await expect(
    pickDirectory({ title: "Select media folder", defaultPath: "/media" }),
  ).resolves.toBe("/media/电影");
  expect(open).toHaveBeenCalledExactlyOnceWith({
    title: "Select media folder",
    defaultPath: "/media",
    directory: true,
    multiple: false,
    recursive: false,
  });
});

it("returns null on cancel without treating it as an error", async () => {
  vi.mocked(open).mockResolvedValue(null);
  await expect(pickDirectory({ title: "Select folder" })).resolves.toBeNull();
  expect(open).toHaveBeenCalledExactlyOnceWith({
    title: "Select folder",
    directory: true,
    multiple: false,
    recursive: false,
  });
});

it("rejects browser usage without invoking the native plugin", async () => {
  vi.mocked(isTauri).mockReturnValue(false);
  await expect(pickDirectory({ title: "Select folder" })).rejects.toMatchObject(
    {
      code: "desktop_required",
    },
  );
  expect(open).not.toHaveBeenCalled();
});

it("redacts native failures instead of exposing paths or secret-bearing errors", async () => {
  vi.mocked(open).mockRejectedValue(
    new Error("Denied /home/private/media, token=synthetic-secret"),
  );
  const error = await pickDirectory({ title: "Select folder" }).catch(
    (failure: unknown) => failure,
  );
  expect(error).toMatchObject({
    code: "request_failed",
    message: "request_failed",
    envelope: undefined,
  });
  expect(JSON.stringify(error)).not.toMatch(/private|synthetic-secret/);
});

it.each([[], ["/media"], "", " ", undefined, 42].map((value) => ({ value })))(
  "rejects malformed or multiple selections ($value)",
  async ({ value }) => {
    vi.mocked(open).mockResolvedValue(value as never);
    await expect(
      pickDirectory({ title: "Select folder" }),
    ).rejects.toMatchObject({
      code: "invalid_response",
    });
  },
);
