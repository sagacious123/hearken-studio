import { useEffect, useRef, useState } from "react";

/**
 * A 6-digit code as six boxes: typing moves on, Backspace moves back, pasting (or the system
 * filling in the code) fills every box. `onChange` gets the digits typed so far; `onComplete`
 * runs once every box is filled. Setting `value` to "" clears the boxes.
 */
export function CodeBoxes({ value, onChange, onComplete, label, length = 6, autoFocus = false }: {
  value: string;
  onChange: (code: string) => void;
  onComplete?: (code: string) => void;
  label: string;
  length?: number;
  autoFocus?: boolean;
}) {
  const [cells, setCells] = useState<string[]>(() => Array.from({ length }, (_, i) => value[i] ?? ""));
  const boxes = useRef<(HTMLInputElement | null)[]>([]);

  useEffect(() => {
    if (value === "") setCells(Array(length).fill(""));
  }, [value, length]);

  const focus = (i: number) => boxes.current[Math.max(0, Math.min(length - 1, i))]?.focus();

  const update = (next: string[]) => {
    setCells(next);
    const code = next.join("");
    onChange(code);
    if (next.every(Boolean)) onComplete?.(code);
  };

  /** Puts `typed` in from box `from` on, then moves to the box after it. */
  const fill = (from: number, typed: string) => {
    const next = [...cells];
    [...typed].forEach((d, k) => from + k < length && (next[from + k] = d));
    update(next);
    focus(from + typed.length);
  };

  return (
    <div role="group" aria-label={label} className="flex gap-2">
      {cells.map((d, i) => (
        <input
          key={i}
          ref={(el) => {
            boxes.current[i] = el;
          }}
          value={d}
          inputMode="numeric"
          autoComplete={i === 0 ? "one-time-code" : "off"}
          autoFocus={autoFocus && i === 0}
          aria-label={`${label}: ${i + 1} / ${length}`}
          maxLength={length}
          onFocus={(e) => e.target.select()}
          onChange={(e) => {
            const typed = e.target.value.replace(/\D/g, "");
            if (typed) {
              // A phone or password manager may put the whole code in one box.
              fill(i, typed.slice(0, length - i));
            } else {
              const next = [...cells];
              next[i] = "";
              update(next);
            }
          }}
          onKeyDown={(e) => {
            if (e.key === "Backspace" && !d && i > 0) {
              e.preventDefault();
              const next = [...cells];
              next[i - 1] = "";
              update(next);
              focus(i - 1);
            } else if (e.key === "ArrowLeft") {
              e.preventDefault();
              focus(i - 1);
            } else if (e.key === "ArrowRight") {
              e.preventDefault();
              focus(i + 1);
            }
          }}
          onPaste={(e) => {
            const typed = e.clipboardData.getData("text").replace(/\D/g, "");
            if (!typed) return;
            e.preventDefault();
            fill(0, typed.slice(0, length));
          }}
          className="h-12 w-11 rounded-lg border border-zinc-800 bg-black/20 text-center font-mono text-xl font-semibold text-zinc-100 caret-amber-500 outline-none focus:border-amber-500 focus:ring-2 focus:ring-amber-500/20"
        />
      ))}
    </div>
  );
}
