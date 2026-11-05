import { Check, ChevronDown } from "lucide-react";
import {
  Children,
  type ComponentProps,
  type CSSProperties,
  isValidElement,
  type KeyboardEvent,
  type ReactNode,
  useId,
  useLayoutEffect,
  useRef,
  useState,
} from "react";
import { createPortal } from "react-dom";
import { cn } from "../../lib/utils";
import { Button } from "./button";

interface Choice {
  value: string;
  label: string;
  disabled: boolean;
  group?: string;
}

function textContent(children: ReactNode): string {
  return Children.toArray(children)
    .map((child) => {
      if (typeof child === "string" || typeof child === "number") {
        return String(child);
      }
      return isValidElement<{ children?: ReactNode }>(child)
        ? textContent(child.props.children)
        : "";
    })
    .join("");
}

function choices(children: ReactNode, disabled = false, group?: string): Choice[] {
  return Children.toArray(children).flatMap((child) => {
    if (!isValidElement<ComponentProps<"option">>(child)) return [];
    if (child.type === "option") {
      const label = child.props.label ?? textContent(child.props.children);
      return [{
        value: String(child.props.value ?? label),
        label,
        disabled: disabled || Boolean(child.props.disabled),
        group,
      }];
    }
    return choices(
      child.props.children,
      disabled || Boolean(child.props.disabled),
      child.type === "optgroup" ? child.props.label : group,
    );
  });
}

type SelectProps = Omit<
  ComponentProps<"select">,
  "multiple" | "size" | "value" | "defaultValue"
> & {
  value?: string | number;
  defaultValue?: string | number;
};

/** Custom shadcn listbox; the hidden native control only bridges forms and change events. */
export function Select({
  children,
  className,
  value,
  defaultValue,
  disabled,
  id,
  onChange,
  onInvalid,
  onFocus,
  ref,
  ...props
}: SelectProps) {
  const generatedId = useId();
  const triggerId = id ?? `${generatedId}-trigger`;
  const listId = `${generatedId}-list`;
  const options = choices(children);
  const initial = String(
    defaultValue ?? options.find((item) => !item.disabled)?.value ?? "",
  );
  const [uncontrolled, setUncontrolled] = useState(initial);
  const selected = value === undefined ? uncontrolled : String(value);
  const selectedIndex = options.findIndex((item) => item.value === selected);
  const [open, setOpen] = useState(false);
  const [active, setActive] = useState(selectedIndex);
  const [label, setLabel] = useState("");
  const [invalid, setInvalid] = useState(false);
  const [position, setPosition] = useState<CSSProperties & Record<`--select-${string}`, string>>({});
  const [portalRoot, setPortalRoot] = useState<HTMLElement | null>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  const native = useRef<HTMLSelectElement>(null);
  const popup = useRef<HTMLDivElement>(null);
  const search = useRef({ text: "", time: 0 });

  useLayoutEffect(() => {
    if (props["aria-label"] || props["aria-labelledby"]) return;
    const labels = Array.from(trigger.current?.labels ?? []).map((element) => {
      const clone = element.cloneNode(true) as HTMLLabelElement;
      for (const control of clone.querySelectorAll(
        "[data-slot=select], [data-slot=select-native]",
      )) control.remove();
      return clone.textContent?.trim() ?? "";
    }).filter(Boolean).join(" ");
    setLabel(labels);
  });

  useLayoutEffect(() => {
    if (disabled) setOpen(false);
    if (!open || disabled) return;
    function place() {
      const rect = trigger.current?.getBoundingClientRect();
      if (!rect) return;
      const container =
        portalRoot && portalRoot !== document.body ? portalRoot : null;
      const bounds = container?.getBoundingClientRect();
      const originLeft = (bounds?.left ?? 0) + (container?.clientLeft ?? 0);
      const originTop = (bounds?.top ?? 0) + (container?.clientTop ?? 0);
      const leftEdge = Math.max(4, originLeft + 4);
      const rightEdge = Math.min(
        window.innerWidth,
        container ? originLeft + container.clientWidth : window.innerWidth,
      ) - 4;
      const below = Math.min(
        window.innerHeight,
        container ? originTop + container.clientHeight : window.innerHeight,
      ) - rect.bottom - 8;
      const above = rect.top - Math.max(0, originTop) - 8;
      const down =
        below >= Math.min(240, options.length * 36 + 8) || below >= above;
      const width = Math.max(
        0,
        Math.min(Math.max(rect.width, 120), rightEdge - leftEdge),
      );
      const offsetLeft = -originLeft + (container?.scrollLeft ?? 0);
      const offsetTop = -originTop + (container?.scrollTop ?? 0);
      setPosition({
        "--select-left": `${Math.max(leftEdge, Math.min(rect.left, rightEdge - width)) + offsetLeft}px`,
        "--select-width": `${width}px`,
        "--select-max-height": `${Math.max(0, Math.min(320, down ? below : above))}px`,
        ...(down
          ? { "--select-top": `${rect.bottom + offsetTop + 4}px` }
          : {
              "--select-bottom":
                `${(container?.clientHeight ?? window.innerHeight) - rect.top - offsetTop + 4}px`,
            }),
      });
    }
    function outside(event: Event) {
      const target = event.target;
      if (
        target instanceof Node &&
        !trigger.current?.contains(target) &&
        !popup.current?.contains(target)
      ) setOpen(false);
    }
    place();
    window.addEventListener("resize", place);
    window.addEventListener("scroll", place, true);
    document.addEventListener("pointerdown", outside, true);
    document.addEventListener("focusin", outside);
    const observer =
      typeof ResizeObserver === "undefined" ? undefined : new ResizeObserver(place);
    if (trigger.current) observer?.observe(trigger.current);
    return () => {
      window.removeEventListener("resize", place);
      window.removeEventListener("scroll", place, true);
      document.removeEventListener("pointerdown", outside, true);
      document.removeEventListener("focusin", outside);
      observer?.disconnect();
    };
  }, [open, disabled, options.length, portalRoot]);

  useLayoutEffect(() => {
    if (open) {
      document.getElementById(`${listId}-${active}`)?.scrollIntoView?.({ block: "nearest" });
    }
  }, [open, active, listId]);

  useLayoutEffect(() => {
    const form = native.current?.form;
    function reset() {
      setUncontrolled(initial);
      setInvalid(false);
      setOpen(false);
    }
    form?.addEventListener("reset", reset);
    return () => form?.removeEventListener("reset", reset);
  }, [initial]);

  function show(index = selectedIndex) {
    if (disabled) return;
    setPortalRoot(
      trigger.current?.closest<HTMLElement>('[role="dialog"]') ?? document.body,
    );
    setActive(
      options[index] && !options[index].disabled
        ? index
        : options.findIndex((item) => !item.disabled),
    );
    setOpen(true);
  }

  function choose(index: number) {
    const option = options[index];
    const control = native.current;
    if (disabled || !option || option.disabled || !control) return;
    setOpen(false);
    trigger.current?.focus();
    if (option.value === selected) return;
    control.value = option.value;
    control.dispatchEvent(new Event("change", { bubbles: true }));
  }

  function keyDown(event: KeyboardEvent<HTMLButtonElement>) {
    if (event.key === "Tab") { setOpen(false); return; }
    if (event.key === "Escape") {
      if (open) { event.preventDefault(); event.stopPropagation(); setOpen(false); }
      return;
    }
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      if (open) choose(active); else show();
      return;
    }
    const enabled = options.map((item, index) => item.disabled ? -1 : index).filter((index) => index >= 0);
    if (!enabled.length) return;
    if (["ArrowDown", "ArrowUp", "Home", "End"].includes(event.key)) {
      event.preventDefault();
      const current = enabled.indexOf(open ? active : selectedIndex);
      const next = event.key === "Home" ? enabled[0]
        : event.key === "End" ? enabled[enabled.length - 1]
        : current < 0 ? (event.key === "ArrowDown" ? enabled[0] : enabled[enabled.length - 1])
        : enabled[(current + (event.key === "ArrowDown" ? 1 : enabled.length - 1) + enabled.length) % enabled.length];
      if (open) setActive(next); else show(next);
      return;
    }
    if (event.key.length === 1 && !event.ctrlKey && !event.metaKey && !event.altKey) {
      event.preventDefault();
      const now = Date.now();
      search.current = { text: now - search.current.time < 700 ? search.current.text + event.key : event.key, time: now };
      const text = search.current.text.toLocaleLowerCase();
      const query = [...text].every((character) => character === text[0]) ? text[0] : text;
      const start = enabled.indexOf(open ? active : selectedIndex);
      const ordered = [...enabled.slice(start + 1), ...enabled.slice(0, start + 1)];
      const next = ordered.find((index) => options[index].label.toLocaleLowerCase().startsWith(query));
      if (next !== undefined) { if (open) setActive(next); else show(next); }
    }
  }

  return (
    <>
      <Button
        ref={trigger}
        id={triggerId}
        data-slot="select"
        className={cn("h-11 w-full min-w-0 justify-between rounded-[10px] border border-input bg-background px-3 text-left text-sm font-normal text-foreground hover:border-ring hover:bg-background focus-visible:ring-ring/40 disabled:pointer-events-auto disabled:cursor-not-allowed disabled:opacity-45 aria-invalid:border-destructive", className)}
        variant="outline"
        role="combobox"
        value={selected}
        disabled={disabled}
        tabIndex={props.tabIndex}
        autoFocus={props.autoFocus}
        aria-label={props["aria-label"] ?? (label || undefined)}
        aria-labelledby={props["aria-labelledby"]}
        aria-describedby={props["aria-describedby"]}
        aria-invalid={props["aria-invalid"] ?? (invalid || undefined)}
        aria-required={props.required || undefined}
        aria-haspopup="listbox"
        aria-expanded={open && !disabled}
        aria-controls={listId}
        aria-activedescendant={open && active >= 0 ? `${listId}-${active}` : undefined}
        title={props.title}
        onClick={() => open ? setOpen(false) : show()}
        onKeyDown={keyDown}
      >
        <span data-slot="select-value" className="min-w-0 flex-1 truncate">{options[selectedIndex]?.label ?? ""}</span>
        <ChevronDown size={16} className="text-muted-foreground" aria-hidden="true" />
      </Button>
      <select
        {...props}
        ref={(element) => {
          native.current = element;
          if (typeof ref === "function") ref(element);
          else if (ref) ref.current = element;
        }}
        id={`${triggerId}-native`}
        data-slot="select-native"
        aria-hidden="true"
        aria-label={undefined}
        aria-labelledby={undefined}
        className="pointer-events-none absolute -m-px size-px overflow-hidden p-0 opacity-0 [clip-path:inset(50%)]"
        tabIndex={-1}
        disabled={disabled}
        value={selected}
        onChange={(event) => {
          setInvalid(false);
          if (value === undefined) setUncontrolled(event.currentTarget.value);
          onChange?.(event);
        }}
        onInvalid={(event) => {
          setInvalid(true);
          onInvalid?.(event);
          event.preventDefault();
          trigger.current?.focus();
          show();
        }}
        onFocus={(event) => { trigger.current?.focus(); onFocus?.(event); }}
      >
        {options.map((option) => (
          <option key={option.value} value={option.value} disabled={option.disabled} />
        ))}
      </select>
      {open && !disabled && portalRoot && createPortal(
        <div ref={popup} id={listId} data-slot="select-content" data-position={portalRoot === document.body ? "viewport" : "dialog"} className="pointer-events-auto fixed top-[var(--select-top,auto)] bottom-[var(--select-bottom,auto)] left-[var(--select-left)] z-80 max-h-[var(--select-max-height)] w-[var(--select-width)] overflow-auto overscroll-contain rounded-[10px] border border-border bg-popover p-1 text-popover-foreground shadow-[0_8px_30px_color-mix(in_srgb,var(--foreground)_12%,transparent)] data-[position=dialog]:absolute" style={position} role="listbox" aria-labelledby={triggerId}>
          {options.map((option, index) => (
            <div key={option.value}>
              {option.group && option.group !== options[index - 1]?.group && <div className="px-2 py-1.5 text-xs font-medium text-muted-foreground">{option.group}</div>}
              <Button
                id={`${listId}-${index}`}
                data-slot="select-item"
                role="option"
                aria-selected={option.value === selected}
                aria-disabled={option.disabled || undefined}
                disabled={option.disabled}
                value={option.value}
                variant="ghost"
                tabIndex={-1}
                className={cn("relative h-auto min-h-9 w-full justify-start rounded-md py-2 pr-8 pl-2 text-left font-normal whitespace-normal text-popover-foreground", active === index && "bg-accent text-accent-foreground")}
                onPointerDown={(event) => event.preventDefault()}
                onPointerMove={() => !option.disabled && setActive(index)}
                onClick={() => choose(index)}
              >
                <span className="flex-1">{option.label}</span>
                {option.value === selected && <Check size={16} className="absolute right-2" aria-hidden="true" />}
              </Button>
            </div>
          ))}
        </div>, portalRoot,
      )}
    </>
  );
}
