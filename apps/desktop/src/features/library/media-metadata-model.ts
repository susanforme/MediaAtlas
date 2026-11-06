export function metadataRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

export function metadataText(value: unknown): string {
  if (typeof value === "string") return value.trim();
  if (typeof value === "number" && Number.isFinite(value)) return String(value);
  if (typeof value === "boolean") return value ? "✓" : "—";
  if (Array.isArray(value)) return value.map(metadataText).filter(Boolean).join(" / ");
  return "";
}

export function castNames(value: unknown): string[] {
  if (!Array.isArray(value)) return [];
  return [...new Set(value.flatMap((person) => {
    if (typeof person === "string") return person.trim() ? [person.trim()] : [];
    if (!metadataRecord(person) || typeof person.name !== "string" || !person.name.trim()) return [];
    const role = typeof person.role === "string" ? person.role.trim() : "";
    return [role ? `${person.name.trim()} (${role})` : person.name.trim()];
  }))];
}

export function metadataRatings(value: unknown) {
  return Array.isArray(value) ? value.filter((item): item is Record<string, unknown> & { name: string; value: number; max: number } =>
    metadataRecord(item) && typeof item.name === "string" && typeof item.value === "number" && Number.isFinite(item.value) &&
    typeof item.max === "number" && Number.isFinite(item.max) && item.max > 0 && item.value >= 0 && item.value <= item.max,
  ) : [];
}

export function hasMetadataValue(key: string, value: unknown): boolean {
  if (key === "nfo_artwork") return false;
  if (key === "cast" || key === "people") return castNames(value).length > 0;
  if (key === "ratings") return metadataRatings(value).length > 0;
  if (key === "collection" && metadataRecord(value)) return !!metadataText(value.name);
  return !!metadataText(value);
}
