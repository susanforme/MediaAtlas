import type { ComponentProps } from "react";
import { cn } from "../../lib/utils";

export function Label({ className, ...props }: ComponentProps<"label">) {
  return (
    <label
      htmlFor={props.htmlFor}
      data-slot="label"
      className={cn("text-sm font-medium", className)}
      {...props}
    >
      {props.children}
    </label>
  );
}
