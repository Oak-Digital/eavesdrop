import { useState } from "react";
import * as SelectPrimitive from "@radix-ui/react-select";

export interface SelectOption {
  value: string;
  label: string;
  description?: string;
  disabled?: boolean;
}

interface SelectProps {
  id?: string;
  label: string;
  value: string;
  options: SelectOption[];
  onValueChange: (value: string) => void;
  placeholder?: string;
  disabled?: boolean;
  autoFocus?: boolean;
}

// Encode every value so an empty application value (e.g. system microphone)
// remains a selectable option; Radix reserves an empty value for placeholders.
const encodeValue = (value: string) => `value:${value}`;

export function Select({ id, label, value, options, onValueChange, placeholder = "Choose an option…", disabled, autoFocus }: SelectProps) {
  const [open, setOpen] = useState(false);
  const selected = options.find((option) => option.value === value);
  return (
    <SelectPrimitive.Root
      value={selected ? encodeValue(value) : ""}
      onValueChange={(next) => onValueChange(next.slice("value:".length))}
      open={open && !disabled && options.length > 0}
      onOpenChange={setOpen}
      disabled={disabled || options.length === 0}
    >
      <SelectPrimitive.Trigger id={id} className="select-trigger" aria-label={label} autoFocus={autoFocus}>
        <span className="select-value">
          <SelectPrimitive.Value placeholder={placeholder}>
            {selected && <>{selected.label}{selected.description && <span className="select-value-description"> · {selected.description}</span>}</>}
          </SelectPrimitive.Value>
        </span>
        <SelectPrimitive.Icon className="select-chevron"><ChevronDown /></SelectPrimitive.Icon>
      </SelectPrimitive.Trigger>
      <SelectPrimitive.Portal>
        <SelectPrimitive.Content
          className="select-content"
          position="popper"
          sideOffset={6}
          collisionPadding={12}
          aria-label={label}
          onKeyDown={(event) => {
            // Escape closes this menu first, without also dismissing a parent dialog.
            if (event.key === "Escape") {
              event.stopPropagation();
              setOpen(false);
            }
          }}
        >
          <SelectPrimitive.ScrollUpButton className="select-scroll select-scroll-up"><ChevronDown /></SelectPrimitive.ScrollUpButton>
          <SelectPrimitive.Viewport className="select-viewport">
            {options.map((option) => (
              <SelectPrimitive.Item className="select-option" key={option.value} value={encodeValue(option.value)} textValue={option.label} disabled={option.disabled}>
                <span className="select-option-copy">
                  <SelectPrimitive.ItemText>{option.label}</SelectPrimitive.ItemText>
                  {option.description && <span className="select-option-description">{option.description}</span>}
                </span>
                <SelectPrimitive.ItemIndicator className="select-check">
                  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" aria-hidden="true"><path d="m5 12 4 4L19 6" /></svg>
                </SelectPrimitive.ItemIndicator>
              </SelectPrimitive.Item>
            ))}
          </SelectPrimitive.Viewport>
          <SelectPrimitive.ScrollDownButton className="select-scroll"><ChevronDown /></SelectPrimitive.ScrollDownButton>
        </SelectPrimitive.Content>
      </SelectPrimitive.Portal>
    </SelectPrimitive.Root>
  );
}

function ChevronDown() {
  return <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><path d="m6 9 6 6 6-6" /></svg>;
}
