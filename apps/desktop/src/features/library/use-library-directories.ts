import { useQuery } from "@tanstack/react-query";
import { adultApi } from "../../api/adult";
import { libraryApi } from "../../api/library";
import { usePrivacy } from "./context";

/** One directory list; private requests remain behind the backend grant. */
export function useLibraryDirectories() {
  const { includeAdult, settings } = usePrivacy();
  const adultVisible = Boolean(settings?.entry_visible);
  const adultAvailable = adultVisible && includeAdult;
  const ordinary = useQuery({
    queryKey: ["roots", false],
    queryFn: () => libraryApi.roots(false),
  });
  const privateRoots = useQuery({
    queryKey: ["adult", "roots"],
    queryFn: adultApi.roots,
    enabled: adultAvailable,
  });
  const roots = {
    ...ordinary,
    data: [
      ...(ordinary.data ?? []).filter((root) => root.kind !== "adult_video"),
      ...(adultAvailable ? (privateRoots.data ?? []) : []),
    ],
    isPending: ordinary.isPending || (adultAvailable && privateRoots.isPending),
    isError: ordinary.isError || (adultAvailable && privateRoots.isError),
    isSuccess:
      ordinary.isSuccess && (!adultAvailable || privateRoots.isSuccess),
    error: ordinary.error ?? (adultAvailable ? privateRoots.error : null),
    refetch: () =>
      Promise.all([
        ordinary.refetch(),
        ...(adultAvailable ? [privateRoots.refetch()] : []),
      ]),
  };
  return { roots, adultAvailable, adultVisible };
}
