import { invoke, isTauri } from "@tauri-apps/api/core";
import { beforeEach, expect, it, vi } from "vitest";
import {
  syntheticMusicGraph,
  syntheticTagPreview,
} from "../features/music/fixtures";
import { musicApi } from "./music";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), isTauri: vi.fn() }));
beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(isTauri).mockReturnValue(true);
});
const reply = (data: unknown) =>
  vi.mocked(invoke).mockResolvedValue({ schema_version: 1, data });
it("imports only an explicit MBID and uses versioned desktop envelopes", async () => {
  reply(syntheticMusicGraph);
  await expect(musicApi.import("mbid-1")).resolves.toEqual(syntheticMusicGraph);
  expect(invoke).toHaveBeenCalledWith("music_import", {
    input: { mbid: "mbid-1" },
  });
});
it("rejects malformed nested tracklists and unsafe source URLs", async () => {
  reply({
    ...syntheticMusicGraph,
    tracks: [{ ...syntheticMusicGraph.tracks[0], position: -1 }],
  });
  await expect(musicApi.album("release-1")).rejects.toMatchObject({
    code: "invalid_response",
  });
  reply({ ...syntheticMusicGraph, source_url: "javascript:alert(1)" });
  await expect(musicApi.release("mbid-1")).rejects.toMatchObject({
    code: "invalid_response",
  });
});
it("passes selected fields and separates preview, execution and restore", async () => {
  reply(syntheticTagPreview);
  await musicApi.preview("file-1", { title: "Online track" });
  expect(invoke).toHaveBeenLastCalledWith("music_tag_preview", {
    input: { file_id: "file-1", changes: { title: "Online track" } },
  });
  reply({ ...syntheticTagPreview, kind: "restore" });
  await musicApi.restorePreview("op-1");
  expect(invoke).toHaveBeenLastCalledWith("music_restore_preview", {
    input: { operation_id: "op-1" },
  });
  expect(
    vi
      .mocked(invoke)
      .mock.calls.some(([command]) => command === "music_tag_execute"),
  ).toBe(false);
});
it("accepts only backend data image thumbnails", async () => {
  reply({
    thumbnail: "https://archive.org/unsafe-direct.jpg",
    source_url: "https://coverartarchive.org/release/id",
    license: "Unknown",
  });
  await expect(
    musicApi.coverPreview("mbid", 0, "release"),
  ).rejects.toMatchObject({ code: "invalid_response" });
  reply({
    thumbnail: "data:image/png;base64,aGVsbG8=",
    source_url: "https://coverartarchive.org/release/id",
    license: "Unknown",
  });
  await expect(
    musicApi.coverPreview("mbid", 0, "release"),
  ).resolves.toMatchObject({ license: "Unknown" });
});
it("rejects unavailable desktop and malformed tag capability metadata", async () => {
  vi.mocked(isTauri).mockReturnValue(false);
  await expect(musicApi.library()).rejects.toMatchObject({
    code: "desktop_required",
  });
  expect(invoke).not.toHaveBeenCalled();
  vi.mocked(isTauri).mockReturnValue(true);
  reply({
    file_id: "file-1",
    tags: { title: "a" },
    format: "FLAC",
    writable: "yes",
    picture_count: 0,
  });
  await expect(musicApi.read("file-1")).rejects.toMatchObject({
    code: "invalid_response",
  });
});
it("requests native opening without claiming playback", async () => {
  reply({ requested: true });
  await expect(musicApi.openFile("file-1")).resolves.toEqual({
    requested: true,
  });
  expect(invoke).toHaveBeenLastCalledWith("music_open_file", {
    input: { file_id: "file-1" },
  });
  reply({ playing: true });
  await expect(musicApi.openFile("file-1")).rejects.toMatchObject({
    code: "invalid_response",
  });
});
it("loads saved covers through local IPC and accepts a missing cover", async () => {
  reply(null);
  await expect(musicApi.cachedCover("release-1")).resolves.toBeNull();
  expect(invoke).toHaveBeenLastCalledWith("music_cached_cover", {
    input: { release_id: "release-1" },
  });
  reply({
    thumbnail: "https://coverartarchive.org/image.jpg",
    source_url: "https://coverartarchive.org/release/id",
    license: "Unknown",
  });
  await expect(musicApi.cachedCover("release-1")).rejects.toMatchObject({
    code: "invalid_response",
  });
});
