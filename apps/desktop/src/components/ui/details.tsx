import type { ComponentProps } from "react";
import { cn } from "../../lib/utils";

export function Details({ className, ...props }: ComponentProps<"details">) {
  return (
    <details
      data-slot="details"
      className={cn("rounded-xl border border-border p-4", className)}
      {...props}
    />
  );
}

export function DetailsSummary({
  className,
  ...props
}: ComponentProps<"summary">) {
  return (
    <summary
      data-slot="details-summary"
      className={cn(
        "cursor-pointer rounded text-sm font-medium focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
        className,
      )}
      {...props}
    />
  );
}
