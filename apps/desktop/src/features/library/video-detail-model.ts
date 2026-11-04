import type { MediaDetail, MetadataValue } from "../../api/library";

/** The library values include field locks and edits; provider snapshots do not override them. */
export function videoDetailFields(
  detail: Pick<MediaDetail, "metadata_values">,
  language: string,
) {
  function field(key: string): MetadataValue | undefined {
    const values = detail.metadata_values.filter((item) => item.field_key === key);
    return (
      values.find((item) => item.locale === language) ??
      values.find((item) => item.locale === language.split("-")[0]) ??
      values.find((item) => !item.locale) ??
      values[0]
    );
  }
  function text(key: string) {
    const value = field(key)?.value;
    return typeof value === "string" ? value.trim() : "";
  }
  function names(key: string) {
    const value = field(key)?.value;
    return Array.isArray(value)
      ? [
          ...new Set(
            value.filter(
              (item): item is string =>
                typeof item === "string" && !!item.trim(),
            ),
          ),
        ]
      : [];
  }
  const runtime = field("runtime_minutes")?.value ?? field("runtime")?.value;
  const rating = field("rating");
  return {
    overview: text("overview") || text("outline"),
    genres: names("genres"),
    cast: names("cast").length ? names("cast") : names("people"),
    directors: names("directors"),
    premiered: text("release_date") || text("first_air_date") || text("premiered"),
    language: text("original_language") || text("language"),
    status: text("status"),
    runtime:
      typeof runtime === "number" && Number.isFinite(runtime) && runtime > 0
        ? runtime
        : null,
    rating:
      typeof rating?.value === "number" &&
      Number.isFinite(rating.value) &&
      rating.value >= 0 &&
      rating.value <= 10
        ? { value: rating.value, provider: rating.provider_id }
        : null,
  };
}
