import { SlidersHorizontal, X } from "lucide-react";
import { type ReactNode, useId, useState } from "react";
import { Button } from "./button";
import "./filter-bar.css";

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
    <section data-slot="filter-bar" className="filter-bar" aria-label={label}>
      <div className="filter-bar-controls">
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
            <span className="filter-bar-count">{activeCount}</span>
          )}
        </Button>
        {actions}
      </div>
      {open && (
        <section id={id} className="filter-bar-options" aria-label={optionsLabel}>
          {filters}
        </section>
      )}
      {summary && <div className="filter-bar-summary">{summary}</div>}
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
      className="filter-bar-chip"
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
