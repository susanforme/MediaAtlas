import type { NfoDocument } from "../../api/nfo";

function text(value: unknown): string {
  if (value == null) return "";
  if (typeof value === "string") return value.trim();
  if (Array.isArray(value)) return value.map(text).filter(Boolean).join(" · ");
  if (typeof value === "object" && Object.keys(value).length === 0) return "";
  return JSON.stringify(value) ?? "";
}

function castText(value: unknown): string {
  if (Array.isArray(value)) return value.map(castText).filter(Boolean).join(" · ");
  if (typeof value === "string") return value.trim();
  if (value !== null && typeof value === "object" && "name" in value && typeof value.name === "string") {
    const role = "role" in value && typeof value.role === "string" ? value.role.trim() : "";
    return role ? `${value.name} (${role})` : value.name;
  }
  return "";
}

export function nfoDetail(document: NfoDocument) {
  const fields = document.metadata.fields;
  const title = typeof fields.title === "string" ? fields.title.trim() : "";
  const originalTitle = typeof fields.original_title === "string" ? fields.original_title.trim() : "";
  const overviewKey = typeof fields.overview === "string" && fields.overview.trim() ? "overview" : "outline";
  const overview = typeof fields[overviewKey] === "string" ? fields[overviewKey].trim() : "";
  const year = typeof fields.year === "number" && Number.isInteger(fields.year) && fields.year > 0 ? fields.year : null;
  const runtime = typeof fields.runtime === "number" && Number.isFinite(fields.runtime) && fields.runtime > 0 ? fields.runtime : null;
  const rating = typeof fields.rating === "number" && Number.isFinite(fields.rating) && fields.rating >= 0 && fields.rating <= 10 ? { value: fields.rating, provider: null } : null;
  const genres = Array.isArray(fields.genres) ? [...new Set(fields.genres.filter((value): value is string => typeof value === "string" && !!value.trim()).map((value) => value.trim()))] : [];
  const shown = new Set(["title", "original_title"]);
  if (overview) shown.add(overviewKey);
  if (year !== null) shown.add("year");
  if (runtime !== null) shown.add("runtime");
  if (rating) shown.add("rating");
  if (genres.length) shown.add("genres");
  if (castText(fields.cast)) shown.add("people");
  return {
    title: title || document.name,
    originalTitle,
    overview,
    year,
    runtime,
    rating,
    genres,
    fields: Object.entries(fields)
      .filter(([key]) => !shown.has(key))
      .map(([key, value]) => ({ key, value: key === "cast" ? castText(value) : text(value) }))
      .filter((field) => field.value !== ""),
  };
}
