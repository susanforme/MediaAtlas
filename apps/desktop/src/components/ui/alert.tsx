import { Slot } from "@radix-ui/react-slot";
import { cva, type VariantProps } from "class-variance-authority";
import type { ComponentProps } from "react";
import { cn } from "../../lib/utils";

const alertVariants = cva("relative w-full rounded-xl border p-4 text-sm", {
  variants: {
    variant: {
      default: "border-border bg-muted text-foreground",
      destructive: "border-destructive/30 bg-destructive/10 text-destructive",
    },
  },
  defaultVariants: { variant: "default" },
});

export function Alert({
  className,
  variant,
  asChild = false,
  ...props
}: ComponentProps<"div"> &
  VariantProps<typeof alertVariants> & { asChild?: boolean }) {
  const Comp = asChild ? Slot : "div";
  return (
    <Comp
      data-slot="alert"
      role={asChild ? undefined : "alert"}
      className={cn(alertVariants({ variant }), className)}
      {...props}
    />
  );
}
