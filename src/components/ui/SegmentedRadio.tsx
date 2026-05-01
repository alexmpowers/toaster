import React from "react";

export interface SegmentedRadioOption<T extends string> {
  value: T;
  label: string;
  disabled?: boolean;
}

interface SegmentedRadioProps<T extends string> {
  value: T;
  options: SegmentedRadioOption<T>[];
  onChange: (value: T) => void;
  ariaLabel?: string;
  disabled?: boolean;
  className?: string;
}

/**
 * Segmented radio control: a row of buttons where exactly one is selected.
 * Use for 2–4 mutually-exclusive options where dropdowns add unnecessary
 * friction (orientation, sample text, font family, etc.).
 *
 * Keyboard: Arrow Left/Right (and Up/Down) move the selection. Tab focuses
 * the active option only — matches WAI-ARIA radiogroup pattern.
 */
export function SegmentedRadio<T extends string>({
  value,
  options,
  onChange,
  ariaLabel,
  disabled,
  className = "",
}: SegmentedRadioProps<T>) {
  const groupRef = React.useRef<HTMLDivElement | null>(null);

  const focusByIndex = (idx: number) => {
    const btns = groupRef.current?.querySelectorAll<HTMLButtonElement>(
      "button[role='radio']:not([disabled])",
    );
    if (!btns || btns.length === 0) return;
    const safe = ((idx % btns.length) + btns.length) % btns.length;
    btns[safe].focus();
  };

  const enabledIndexOf = (val: T) => {
    const enabled = options.filter((o) => !o.disabled);
    return Math.max(
      0,
      enabled.findIndex((o) => o.value === val),
    );
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLDivElement>) => {
    if (disabled) return;
    const enabled = options.filter((o) => !o.disabled);
    if (enabled.length === 0) return;
    const cur = enabledIndexOf(value);
    let next = cur;
    if (e.key === "ArrowRight" || e.key === "ArrowDown") next = cur + 1;
    else if (e.key === "ArrowLeft" || e.key === "ArrowUp") next = cur - 1;
    else if (e.key === "Home") next = 0;
    else if (e.key === "End") next = enabled.length - 1;
    else return;
    e.preventDefault();
    const safe = ((next % enabled.length) + enabled.length) % enabled.length;
    onChange(enabled[safe].value);
    focusByIndex(safe);
  };

  return (
    <div
      ref={groupRef}
      role="radiogroup"
      aria-label={ariaLabel}
      aria-disabled={disabled || undefined}
      onKeyDown={handleKeyDown}
      className={`inline-flex w-full items-stretch gap-1 rounded-lg border border-mid-gray/30 bg-mid-gray/10 p-1 ${className}`}
    >
      {options.map((opt) => {
        const selected = opt.value === value;
        const isDisabled = disabled || opt.disabled;
        return (
          <button
            key={opt.value}
            type="button"
            role="radio"
            aria-checked={selected}
            disabled={isDisabled}
            tabIndex={selected ? 0 : -1}
            onClick={() => {
              if (!isDisabled && opt.value !== value) onChange(opt.value);
            }}
            className={[
              "flex-1 cursor-pointer rounded-md border px-3 py-1.5 text-sm font-medium transition-colors",
              "focus:outline-none focus-visible:ring-1 focus-visible:ring-logo-primary",
              "disabled:cursor-not-allowed disabled:opacity-50",
              selected
                ? "border-logo-primary bg-logo-primary/20 text-text"
                : "border-transparent text-text/70 hover:bg-mid-gray/20 hover:text-text",
            ].join(" ")}
          >
            {opt.label}
          </button>
        );
      })}
    </div>
  );
}
