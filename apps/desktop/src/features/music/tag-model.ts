import type { MusicReleaseGraph } from "../../api/music";

export const tagFields = [
  "title",
  "artist",
  "album",
  "album_artist",
  "track_number",
  "disc_number",
  "date",
  "musicbrainz_release_id",
  "musicbrainz_release_group_id",
  "musicbrainz_track_id",
  "musicbrainz_recording_id",
  "musicbrainz_artist_id",
  "isrc",
  "barcode",
  "catalog_number",
] as const;
export function tagDefaults(
  graph: MusicReleaseGraph,
  fileId: string,
): Record<string, string> {
  const linked = graph.files.find((f) => f.file_id === fileId);
  const track = graph.tracks.find(
    (tr) => tr.media_item_id === linked?.track_id,
  );
  const medium = graph.media.find((m) => m.id === track?.medium_id);
  const identifier = (mediaId: string | undefined, kind: string) =>
    graph.identifiers.find(
      (id) => id.media_item_id === mediaId && id.kind === kind,
    )?.value ?? "";
  return {
    title: track?.title ?? "",
    artist:
      track?.artist_credits
        .map((c) => `${c.credited_name}${c.join_phrase}`)
        .join("") ?? "",
    album: graph.release.title,
    album_artist: graph.release.artist_credits
      .map((c) => `${c.credited_name}${c.join_phrase}`)
      .join(""),
    track_number: String(track?.position ?? ""),
    disc_number: String(medium?.position ?? ""),
    date: graph.release_date_text ?? graph.release.release_date ?? "",
    musicbrainz_release_id: identifier(
      graph.release.media_item_id,
      "music_brainz",
    ),
    musicbrainz_release_group_id: identifier(
      graph.release_group.media_item_id,
      "music_brainz",
    ),
    musicbrainz_track_id: identifier(track?.media_item_id, "music_brainz"),
    musicbrainz_recording_id: identifier(track?.recording_id, "music_brainz"),
    musicbrainz_artist_id: identifier(
      track?.artist_credits[0]?.artist_id ??
        graph.release.artist_credits[0]?.artist_id,
      "music_brainz",
    ),
    isrc: identifier(track?.recording_id, "isrc"),
    barcode:
      identifier(graph.release.media_item_id, "upc") ||
      identifier(graph.release.media_item_id, "ean"),
    catalog_number: identifier(graph.release.media_item_id, "catalog_number"),
  };
}
