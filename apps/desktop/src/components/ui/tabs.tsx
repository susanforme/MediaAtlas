import { type ComponentProps, createContext, useContext, useId } from "react";
import { cn } from "../../lib/utils";

const TabsContext = createContext<{
  id: string;
  value: string;
  onValueChange: (value: string) => void;
} | null>(null);

function useTabs() {
  const tabs = useContext(TabsContext);
  if (!tabs) throw new Error("Tabs components require Tabs");
  return tabs;
}

export function Tabs({
  value,
  onValueChange,
  ...props
}: ComponentProps<"div"> & {
  value: string;
  onValueChange: (value: string) => void;
}) {
  const id = useId();
  return (
    <TabsContext.Provider value={{ id, value, onValueChange }}>
      <div data-slot="tabs" {...props} />
    </TabsContext.Provider>
  );
}

export function TabsList({ className, ...props }: ComponentProps<"div">) {
  return (
    <div
      data-slot="tabs-list"
      role="tablist"
      className={cn(
        "inline-flex items-center gap-1 rounded-lg bg-muted p-1",
        className,
      )}
      {...props}
    />
  );
}

export function TabsTrigger({
  value,
  className,
  onKeyDown,
  ...props
}: ComponentProps<"button"> & { value: string }) {
  const tabs = useTabs();
  const selected = tabs.value === value;
  return (
    <button
      {...props}
      type="button"
      role="tab"
      data-slot="tabs-trigger"
      data-state={selected ? "active" : "inactive"}
      id={`${tabs.id}-tab-${value}`}
      aria-controls={`${tabs.id}-panel-${value}`}
      aria-selected={selected}
      tabIndex={selected ? 0 : -1}
      className={cn(
        "rounded-md px-3 py-1.5 text-sm text-muted-foreground transition-colors data-[state=active]:bg-surface data-[state=active]:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:opacity-45",
        className,
      )}
      onClick={() => tabs.onValueChange(value)}
      onKeyDown={(event) => {
        onKeyDown?.(event);
        if (event.defaultPrevented) return;
        const items = Array.from(
          event.currentTarget
            .closest('[role="tablist"]')
            ?.querySelectorAll<HTMLButtonElement>(
              '[role="tab"]:not(:disabled)',
            ) ?? [],
        );
        const index = items.indexOf(event.currentTarget);
        const next =
          event.key === "ArrowRight"
            ? (index + 1) % items.length
            : event.key === "ArrowLeft"
              ? (index + items.length - 1) % items.length
              : event.key === "Home"
                ? 0
                : event.key === "End"
                  ? items.length - 1
                  : -1;
        if (next < 0) return;
        event.preventDefault();
        items[next]?.focus();
        items[next]?.click();
      }}
    />
  );
}

export function TabsContent({
  value,
  className,
  ...props
}: ComponentProps<"div"> & { value: string }) {
  const tabs = useTabs();
  return (
    <div
      data-slot="tabs-content"
      id={`${tabs.id}-panel-${value}`}
      role="tabpanel"
      aria-labelledby={`${tabs.id}-tab-${value}`}
      hidden={tabs.value !== value}
      // biome-ignore lint/a11y/noNoninteractiveTabindex: APG tab panels need a keyboard stop when their content has no controls.
      tabIndex={0}
      className={cn(
        "min-w-0 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
        className,
      )}
      {...props}
    />
  );
}
