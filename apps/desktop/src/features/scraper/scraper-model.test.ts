import { expect, it } from "vitest";
import { value } from "./scraper-model";

it("shows actor names and roles without exposing image URLs or dropping legacy names", () => {
  expect(value([
    { name: "Actor A", role: "Detective", thumb: "https://image.tmdb.org/a.jpg", tmdb_id: "123" },
    { name: "Actor B", role: "" },
    "Legacy actor",
  ])).toBe("Actor A (Detective)、Actor B、Legacy actor");
  expect(value({ name: "Film collection", tmdb_id: "456" })).toBe("Film collection");
  expect(value(["Drama", "Mystery"])).toBe("Drama、Mystery");
});
