import { Slot } from "@radix-ui/react-slot";
import type { ComponentProps } from "react";
import { cn } from "../../lib/utils";

export function Card({
  className,
  asChild = false,
  ...props
}: ComponentProps<"div"> & { asChild?: boolean }) {
  const Comp = asChild ? Slot : "div";
  return (
    <Comp
      data-slot="card"
      className={cn(
        "min-w-0 rounded-[14px] border border-border bg-card p-[18px] text-card-foreground sm:p-6",
        className,
      )}
      {...props}
    />
  );
}
