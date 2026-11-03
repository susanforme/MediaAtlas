import { Slot } from "@radix-ui/react-slot";
import { cva, type VariantProps } from "class-variance-authority";
import type { ComponentProps } from "react";
import { cn } from "../../lib/utils";

const buttonVariants = cva(
  "inline-flex shrink-0 items-center justify-center gap-2 whitespace-nowrap rounded-[10px] text-sm font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-background disabled:pointer-events-none disabled:opacity-45 [&_svg]:shrink-0",
  {
    variants: {
      variant: {
        default: "bg-primary text-primary-foreground hover:bg-primary/85",
        outline: "border border-border bg-surface hover:bg-hover",
        secondary: "bg-white/8 text-foreground hover:bg-white/13",
        ghost: "text-muted-foreground hover:bg-white/6 hover:text-foreground",
        link: "text-ring underline-offset-4 hover:underline",
        destructive: "bg-destructive text-white hover:bg-destructive/85",
      },
      size: {
        default: "h-11 px-5",
        sm: "h-9 px-3",
        icon: "size-10 p-0",
      },
    },
    defaultVariants: { variant: "default", size: "default" },
  },
);

export function Button({
  className,
  variant,
  size,
  asChild = false,
  ...props
}: ComponentProps<"button"> &
  VariantProps<typeof buttonVariants> & { asChild?: boolean }) {
  const Comp = asChild ? Slot : "button";
  return (
    <Comp
      data-slot="button"
      {...(!asChild && { type: "button" })}
      className={cn(buttonVariants({ variant, size }), className)}
      {...props}
    />
  );
}

export function Anchor({ className, ...props }: ComponentProps<"a">) {
  return (
    <Button
      asChild
      variant="link"
      className={cn("h-auto justify-start p-0 whitespace-normal", className)}
    >
      <a {...props} />
    </Button>
  );
}
