import { type MetadataValue, request } from "./library";

export interface ArtistCredit {
  artist_id: string;
  credited_name: string;
  join_phrase: string;
}
export interface MusicArtist {
  media_item_id: string;
  name: string;
  sort_name: string | null;
  aliases: string[];
}
export interface ReleaseGroup {
  media_item_id: string;
  title: string;
  artist_credits: ArtistCredit[];
  primary_type: string | null;
  secondary_types: string[];
  first_release_date: string | null;
}
export interface Release {
  media_item_id: string;
  release_group_id: string;
  title: string;
  artist_credits: ArtistCredit[];
  country: string | null;
  release_date: string | null;
  status: string | null;
  labels: string[];
}
export interface Medium {
  id: string;
  release_id: string;
  position: number;
  title: string | null;
  format: string | null;
}
export interface Track {
  media_item_id: string;
  medium_id: string;
  recording_id: string;
  position: number;
  number: string;
  title: string;
  length_ms: number | null;
  artist_credits: ArtistCredit[];
}
export interface Recording {
  media_item_id: string;
  title: string;
  length_ms: number | null;
  artist_credits: ArtistCredit[];
}
export interface MusicIdentifier {
  media_item_id: string;
  kind: string;
  namespace: string;
  value: string;
}
export interface TrackFile {
  track_id: string;
  file_id: string;
  relative_path: string;
}
export interface MusicFieldSource extends MetadataValue {
  media_item_id: string;
}
export interface MusicReleaseGraph {
  artists: MusicArtist[];
  release_group: ReleaseGroup;
  release: Release;
  media: Medium[];
  tracks: Track[];
  recordings: Recording[];
  identifiers: MusicIdentifier[];
  files: TrackFile[];
  source_url: string;
  fetched_at: string;
  locale: string | null;
  field_sources?: MusicFieldSource[];
  release_date_text?: string | null;
  release_disambiguation?: string | null;
  group_first_release_date_text?: string | null;
}
export interface MusicAlbumView {
  release_id: string;
  title: string;
  artist: string;
  country: string | null;
  date: string | null;
  medium_count: number;
  track_count: number;
}
export interface MusicArtistView {
  artist: MusicArtist;
  releases: MusicAlbumView[];
}
export interface MusicSearchCandidate {
  mbid: string;
  title: string;
  artist: string;
  country: string | null;
  date: string | null;
  disambiguation: string | null;
  medium_count: number;
  track_count: number;
}
export interface MusicCoverPreview {
  thumbnail: string;
  source_url: string;
  license: string;
}
export interface MusicCover {
  url: string;
  source_url: string;
  license: string;
}
export interface TagRead {
  file_id: string;
  tags: Record<string, string>;
  format: string;
  writable: boolean;
  picture_count: number;
}
export interface TagChange {
  field: string;
  before: string | null;
  after: string | null;
}
export interface TagPreview {
  id: string;
  file_id: string;
  changes: TagChange[];
  kind: "write" | "restore";
  state: string;
}
export interface TagResult {
  operation_id: string;
  file_id: string;
  state: string;
  backup_relative: string | null;
  error_code: string | null;
}
export interface MusicLocalFile {
  file_id: string;
  media_item_id: string;
  relative_path: string;
}
const obj = (v: unknown): v is Record<string, unknown> =>
  v !== null && typeof v === "object" && !Array.isArray(v);
const str = (v: unknown): v is string =>
  typeof v === "string" && v.length <= 2097152;
const nullable = (v: unknown): v is string | null => v === null || str(v);
const num = (v: unknown): v is number =>
  typeof v === "number" && Number.isSafeInteger(v) && v >= 0;
const arr = <T>(v: unknown, g: (v: unknown) => v is T): v is T[] =>
  Array.isArray(v) && v.length <= 10000 && v.every(g);
const https = (v: unknown): v is string => {
  if (!str(v)) return false;
  try {
    const u = new URL(v);
    return u.protocol === "https:" && !u.username && !u.password;
  } catch {
    return false;
  }
};
const credit = (v: unknown): v is ArtistCredit =>
  obj(v) && str(v.artist_id) && str(v.credited_name) && str(v.join_phrase);
const artist = (v: unknown): v is MusicArtist =>
  obj(v) &&
  str(v.media_item_id) &&
  str(v.name) &&
  nullable(v.sort_name) &&
  arr(v.aliases, str);
const album = (v: unknown): v is MusicAlbumView =>
  obj(v) &&
  str(v.release_id) &&
  str(v.title) &&
  str(v.artist) &&
  nullable(v.country) &&
  nullable(v.date) &&
  num(v.medium_count) &&
  num(v.track_count);
export function musicGraph(v: unknown): v is MusicReleaseGraph {
  if (!obj(v) || !obj(v.release_group) || !obj(v.release)) return false;
  const g = v.release_group,
    r = v.release;
  return (
    arr(v.artists, artist) &&
    str(g.media_item_id) &&
    str(g.title) &&
    arr(g.artist_credits, credit) &&
    nullable(g.primary_type) &&
    arr(g.secondary_types, str) &&
    nullable(g.first_release_date) &&
    str(r.media_item_id) &&
    str(r.release_group_id) &&
    str(r.title) &&
    arr(r.artist_credits, credit) &&
    nullable(r.country) &&
    nullable(r.release_date) &&
    nullable(r.status) &&
    arr(r.labels, str) &&
    arr(
      v.media,
      (m): m is Medium =>
        obj(m) &&
        str(m.id) &&
        str(m.release_id) &&
        num(m.position) &&
        nullable(m.title) &&
        nullable(m.format),
    ) &&
    arr(
      v.tracks,
      (t): t is Track =>
        obj(t) &&
        str(t.media_item_id) &&
        str(t.medium_id) &&
        str(t.recording_id) &&
        num(t.position) &&
        str(t.number) &&
        str(t.title) &&
        (t.length_ms === null || num(t.length_ms)) &&
        arr(t.artist_credits, credit),
    ) &&
    arr(
      v.recordings,
      (r): r is Recording =>
        obj(r) &&
        str(r.media_item_id) &&
        str(r.title) &&
        (r.length_ms === null || num(r.length_ms)) &&
        arr(r.artist_credits, credit),
    ) &&
    arr(
      v.identifiers,
      (i): i is MusicIdentifier =>
        obj(i) &&
        str(i.media_item_id) &&
        str(i.kind) &&
        str(i.namespace) &&
        str(i.value),
    ) &&
    arr(
      v.files,
      (f): f is TrackFile =>
        obj(f) && str(f.track_id) && str(f.file_id) && str(f.relative_path),
    ) &&
    https(v.source_url) &&
    str(v.fetched_at) &&
    Number.isFinite(Date.parse(v.fetched_at)) &&
    nullable(v.locale) &&
    (v.field_sources === undefined ||
      arr(
        v.field_sources,
        (f): f is MusicFieldSource =>
          obj(f) &&
          str(f.id) &&
          str(f.media_item_id) &&
          str(f.field_key) &&
          nullable(f.locale) &&
          "value" in f &&
          nullable(f.provider_id) &&
          (f.source_url === null || https(f.source_url)) &&
          typeof f.is_locked === "boolean" &&
          typeof f.is_user_modified === "boolean",
      )) &&
    (v.release_date_text === undefined || nullable(v.release_date_text)) &&
    (v.release_disambiguation === undefined ||
      nullable(v.release_disambiguation)) &&
    (v.group_first_release_date_text === undefined ||
      nullable(v.group_first_release_date_text))
  );
}
export const tagPreview = (v: unknown): v is TagPreview =>
  obj(v) &&
  str(v.id) &&
  str(v.file_id) &&
  arr(
    v.changes,
    (c): c is TagChange =>
      obj(c) && str(c.field) && nullable(c.before) && nullable(c.after),
  ) &&
  (v.kind === "write" || v.kind === "restore") &&
  str(v.state);
const result = (v: unknown): v is TagResult =>
  obj(v) &&
  str(v.operation_id) &&
  str(v.file_id) &&
  str(v.state) &&
  nullable(v.backup_relative) &&
  nullable(v.error_code);
const nil = (v: unknown): v is null => v === null;
const coverImage = (v: unknown): v is MusicCoverPreview =>
  obj(v) &&
  str(v.thumbnail) &&
  /^data:image\/(png|jpeg|webp);base64,[A-Za-z0-9+/]+=*$/.test(v.thumbnail) &&
  https(v.source_url) &&
  str(v.license);
export const musicApi = {
  openFile: (fileId: string) =>
    request(
      "music_open_file",
      { file_id: fileId },
      (value): value is { requested: boolean } =>
        obj(value) && typeof value.requested === "boolean",
    ),
  library: () =>
    request("music_library", {}, (v): v is MusicAlbumView[] => arr(v, album)),
  album: (releaseId: string) =>
    request("music_album", { release_id: releaseId }, musicGraph),
  artist: (artistId: string) =>
    request(
      "music_artist",
      { artist_id: artistId },
      (v): v is MusicArtistView =>
        obj(v) && artist(v.artist) && arr(v.releases, album),
    ),
  search: (query: string) =>
    request("music_search", { query }, (v): v is MusicSearchCandidate[] =>
      arr(
        v,
        (c): c is MusicSearchCandidate =>
          obj(c) &&
          str(c.mbid) &&
          str(c.title) &&
          str(c.artist) &&
          nullable(c.country) &&
          nullable(c.date) &&
          nullable(c.disambiguation) &&
          num(c.medium_count) &&
          num(c.track_count),
      ),
    ),
  release: (mbid: string) => request("music_release", { mbid }, musicGraph),
  import: (mbid: string) => request("music_import", { mbid }, musicGraph),
  localFiles: () =>
    request("music_local_files", {}, (v): v is MusicLocalFile[] =>
      arr(
        v,
        (f): f is MusicLocalFile =>
          obj(f) &&
          str(f.file_id) &&
          str(f.media_item_id) &&
          str(f.relative_path),
      ),
    ),
  history: (fileId: string) =>
    request("music_tag_history", { file_id: fileId }, (v): v is TagResult[] =>
      arr(v, result),
    ),
  cover: (mbid: string) =>
    request("music_cover", { mbid }, (v): v is MusicCover[] =>
      arr(
        v,
        (c): c is MusicCover =>
          obj(c) && https(c.url) && https(c.source_url) && str(c.license),
      ),
    ),
  cachedCover: (releaseId: string) =>
    request(
      "music_cached_cover",
      { release_id: releaseId },
      (v): v is MusicCoverPreview | null => v === null || coverImage(v),
    ),
  coverPreview: (mbid: string, coverIndex: number, releaseId: string) =>
    request(
      "music_cover_preview",
      { mbid, cover_index: coverIndex, release_id: releaseId },
      coverImage,
    ),
  linkTrack: (trackId: string, fileId: string) =>
    request("music_link_track", { track_id: trackId, file_id: fileId }, nil),
  read: (fileId: string) =>
    request(
      "music_tag_read",
      { file_id: fileId },
      (v): v is TagRead =>
        obj(v) &&
        str(v.file_id) &&
        obj(v.tags) &&
        Object.keys(v.tags).length <= 1000 &&
        Object.values(v.tags).every(str) &&
        str(v.format) &&
        typeof v.writable === "boolean" &&
        num(v.picture_count),
    ),
  preview: (fileId: string, changes: Record<string, string>) =>
    request("music_tag_preview", { file_id: fileId, changes }, tagPreview),
  execute: (previewId: string) =>
    request("music_tag_execute", { preview_id: previewId }, result),
  restorePreview: (operationId: string) =>
    request("music_restore_preview", { operation_id: operationId }, tagPreview),
  restoreExecute: (previewId: string) =>
    request("music_restore_execute", { preview_id: previewId }, result),
  discard: (previewId: string) =>
    request("music_tag_discard", { preview_id: previewId }, nil),
};
