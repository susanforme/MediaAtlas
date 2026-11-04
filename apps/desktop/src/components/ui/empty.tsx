import type { ComponentProps } from "react";
import { cn } from "../../lib/utils";

export function Empty({ className, ...props }: ComponentProps<"div">) {
  return (
    <div
      data-slot="empty"
      className={cn(
        "flex min-w-0 flex-col items-center rounded-xl border border-dashed border-border bg-muted/30 px-6 py-14 text-center",
        className,
      )}
      {...props}
    />
  );
}
