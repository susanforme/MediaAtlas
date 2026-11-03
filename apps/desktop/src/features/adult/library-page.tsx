import { LibraryPage } from "../library/browse";
import type { LibrarySearch } from "../library/browse-model";
import { AdultGate } from "./gate";

export function AdultLibraryPage({
  search,
  onSearch,
}: {
  search: LibrarySearch;
  onSearch: (patch: LibrarySearch, replace?: boolean) => void;
}) {
  return (
    <AdultGate>
      <LibraryPage adult search={search} onSearch={onSearch} />
    </AdultGate>
  );
}
