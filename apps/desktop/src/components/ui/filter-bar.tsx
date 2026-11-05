import { SlidersHorizontal, X } from "lucide-react";
import { type ReactNode, useId, useState } from "react";
import { Button } from "./button";

export function FilterBar({
  label,
  optionsLabel,
  activeCount = 0,
  children,
  filters,
  actions,
  summary,
}: {
  label: string;
  optionsLabel: string;
  activeCount?: number;
  children: ReactNode;
  filters: ReactNode;
  actions?: ReactNode;
  summary?: ReactNode;
}) {
  const [open, setOpen] = useState(false);
  const id = useId();
  return (
    <section data-slot="filter-bar" className="mb-5 min-w-0" aria-label={label}>
      <div className="flex min-w-0 flex-wrap items-center gap-2 [&>[data-slot=label]]:block [&>[data-slot=label]]:min-w-0 [&>[data-slot=label]]:flex-[0_1_164px] [&_[data-slot=select]]:h-9 [&_[data-slot=select]]:rounded-lg [&_[data-slot=select]]:bg-transparent [&_[data-slot=select]]:py-1.5 [&_[data-slot=input]]:h-9 [&_[data-slot=input]]:rounded-lg [&_[data-slot=input]]:bg-transparent [&_[data-slot=input]]:py-1.5 [@media(max-width:640px)]:[&>[data-slot=label]]:flex-[1_1_120px] [&>[data-slot=library-search]]:flex-[1_1_200px] [&>[data-slot=library-search]]:max-w-[360px] [&>[data-slot=library-search]>[data-slot=button]]:size-8 [@media(max-width:760px)]:[&>[data-slot=library-search]]:basis-full [@media(max-width:760px)]:[&>[data-slot=library-search]]:max-w-none">
        {children}
        <Button
          data-slot="filter-toggle"
          variant={open || activeCount ? "secondary" : "ghost"}
          size="sm"
          aria-expanded={open}
          aria-controls={open ? id : undefined}
          onClick={() => setOpen(!open)}
        >
          <SlidersHorizontal size={15} aria-hidden="true" />
          {label}
          {activeCount > 0 && (
            <span className="min-w-[18px] rounded bg-primary px-1 text-center text-[11px] text-primary-foreground">{activeCount}</span>
          )}
        </Button>
        {actions}
      </div>
      {open && (
        <section id={id} className="mt-3 grid grid-cols-[repeat(auto-fit,minmax(min(100%,160px),200px))] gap-3 border-t border-border pt-3 [&>[data-slot=label]]:min-w-0 [&>[data-slot=label]]:gap-[5px] [&>[data-slot=label]]:text-[12px] [&>[data-slot=label]]:text-muted-foreground [&_[data-slot=select]]:h-9 [&_[data-slot=select]]:rounded-lg [&_[data-slot=select]]:bg-transparent [&_[data-slot=select]]:py-1.5 [@media(max-width:640px)]:grid-cols-2" aria-label={optionsLabel}>
          {filters}
        </section>
      )}
      {summary && <div className="mt-2.5 flex flex-wrap gap-1.5">{summary}</div>}
    </section>
  );
}

export function FilterChip({
  label,
  children,
  onRemove,
}: {
  label: string;
  children: ReactNode;
  onRemove: () => void;
}) {
  return (
    <Button
      variant="outline"
      size="sm"
      className="h-7 min-w-0 max-w-full gap-1.5 rounded-md bg-transparent px-2 text-[12px] leading-[var(--text-sm--line-height)] font-normal [&:hover]:bg-transparent [&>span]:truncate"
      aria-label={label}
      onClick={(event) => {
        event.currentTarget
          .closest('[data-slot="filter-bar"]')
          ?.querySelector<HTMLButtonElement>('[data-slot="filter-toggle"]')
          ?.focus();
        onRemove();
      }}
    >
      <span>{children}</span>
      <X size={13} aria-hidden="true" />
    </Button>
  );
}
