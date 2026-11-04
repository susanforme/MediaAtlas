import type { ComponentProps } from "react";
import { cn } from "../../lib/utils";
import "./select.css";

/** Shared shadcn native select: app styling with the platform's form semantics. */
export function Select({ className, ...props }: ComponentProps<"select">) {
  return (
    <select
      data-slot="select"
      className={cn(
        "ui-select h-11 w-full min-w-0 appearance-none truncate rounded-[10px] border border-input bg-background py-2 pr-10 pl-3 text-sm text-foreground transition-[border-color,background-color,box-shadow] hover:border-ring focus-visible:border-ring focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/40 disabled:cursor-not-allowed disabled:opacity-45 aria-invalid:border-destructive aria-invalid:focus-visible:ring-destructive/30",
        className,
      )}
      {...props}
    />
  );
}
