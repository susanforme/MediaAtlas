import { useQuery } from "@tanstack/react-query";
import { usePrivacy } from "./context";
import { rootsQuery } from "./queries";

/** One directory list; private requests remain behind the backend grant. */
export function useLibraryDirectories() {
  const { includeAdult, settings } = usePrivacy();
  const adultVisible = Boolean(settings?.entry_visible);
  const adultAvailable = adultVisible && includeAdult;
  const ordinary = useQuery(rootsQuery());
  const privateRoots = useQuery({
    ...rootsQuery(true),
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
