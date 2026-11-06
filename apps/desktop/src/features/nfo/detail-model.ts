import type { NfoDocument } from "../../api/nfo";
import { castNames, hasMetadataValue, metadataText } from "../library/media-metadata-model";

export function nfoDetail(document: NfoDocument) {
  const fields = document.metadata.fields;
  const title = typeof fields.title === "string" ? fields.title.trim() : "";
  const originalTitle = typeof fields.original_title === "string" ? fields.original_title.trim() : "";
  const overview = typeof fields.overview === "string" && fields.overview.trim() ? fields.overview.trim() : typeof fields.outline === "string" ? fields.outline.trim() : "";
  const year = typeof fields.year === "number" && Number.isInteger(fields.year) && fields.year > 0 ? fields.year : null;
  const runtime = typeof fields.runtime === "number" && Number.isFinite(fields.runtime) && fields.runtime > 0 ? fields.runtime : null;
  const rating = typeof fields.rating === "number" && Number.isFinite(fields.rating) && fields.rating >= 0 && fields.rating <= 10 ? { value: fields.rating, provider: null } : null;
  const genres = Array.isArray(fields.genres) ? [...new Set(fields.genres.filter((value): value is string => typeof value === "string" && !!value.trim()).map((value) => value.trim()))] : [];
  const cast = castNames(fields.cast).length ? castNames(fields.cast) : castNames(fields.people);
  return {
    title: title || document.name, originalTitle, overview, year, runtime, rating, genres, cast,
    facts: ["directors", "premiered", "aired", "studios", "countries", "mpaa"]
      .map((key) => ({ key, value: metadataText(fields[key]) })).filter((field) => field.value),
    fields: Object.entries(fields)
      .filter(([key, value]) => !(key === "people" && castNames(fields.cast).length) && hasMetadataValue(key, value))
      .map(([key, value]) => ({ key, value })),
  };
}
