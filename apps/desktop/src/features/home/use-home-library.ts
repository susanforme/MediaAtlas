import { useQuery } from "@tanstack/react-query";
import { defaultBrowse, libraryApi } from "../../api/library";
import { useLibraryActivity } from "../library/use-library-activity";
import { ordinaryCards } from "./media-model";

export function useHomeLibrary() {
  const activity = useLibraryActivity();
  const summary = useQuery({
    queryKey: ["summary", false],
    queryFn: () => libraryApi.summary(defaultBrowse(false)),
  });
  const query = { ...defaultBrowse(false), sort: "added_desc", limit: 14 };
  const media = useQuery({
    queryKey: ["media", query],
    queryFn: () => libraryApi.media(query),
  });
  const cards = ordinaryCards(media.data?.items ?? []);
  const featured = cards[0];
  const detail = useQuery({
    queryKey: ["detail", featured?.media.id, false],
    queryFn: () => libraryApi.detail(featured?.media.id ?? "", false),
    enabled: Boolean(featured),
  });
  return { summary, media, cards, featured, detail, activity };
}
