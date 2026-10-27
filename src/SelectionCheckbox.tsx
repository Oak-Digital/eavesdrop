import { useLayoutEffect, useRef } from "react";

export function SelectionCheckbox({ label, checked, indeterminate = false, disabled = false, onChange }: {
  label: string;
  checked: boolean;
  indeterminate?: boolean;
  disabled?: boolean;
  onChange: () => void;
}) {
  const inputRef = useRef<HTMLInputElement>(null);
  useLayoutEffect(() => {
    if (inputRef.current) inputRef.current.indeterminate = indeterminate;
  }, [indeterminate]);

  return (
    <label className="selection-checkbox">
      <input ref={inputRef} type="checkbox" aria-label={label} checked={checked} disabled={disabled} onChange={onChange} />
      <span className="checkbox-mark" aria-hidden="true">
        <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.75" strokeLinecap="round" strokeLinejoin="round">
          <path className="checkbox-check" d="m3.5 8 3 3 6-6" />
          <path className="checkbox-mixed" d="M4 8h8" />
        </svg>
      </span>
    </label>
  );
}
