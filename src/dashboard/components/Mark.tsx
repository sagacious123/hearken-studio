import { useId } from "react";

/**
 * Hearken's mark: a carved capital H whose crossbar is the colon of a reference (3:16).
 * While listening, the two dots breathe.
 */
export function Mark({ className, listening = false }: { className?: string; listening?: boolean }) {
  // React's ids hold ":" or "«»", which WebKit may not resolve inside url(#…).
  const gold = `mark-gold-${useId().replace(/[^\w-]/g, "")}`;
  return (
    <svg viewBox="0 0 100 100" className={className} aria-hidden>
      <defs>
        <linearGradient id={gold} gradientUnits="userSpaceOnUse" x1="30" y1="6" x2="70" y2="94">
          <stop offset="0" stopColor="#fbe7b5" />
          <stop offset=".48" stopColor="#e9b24c" />
          <stop offset="1" stopColor="#b07a24" />
        </linearGradient>
      </defs>
      <g fill={`url(#${gold})`}>
        <path d="M11 10H39V14.5C35 14.5 33 16.5 33 20.5V79.5C33 83.5 35 85.5 39 85.5V90H11V85.5C15 85.5 17 83.5 17 79.5V20.5C17 16.5 15 14.5 11 14.5Z" />
        <path d="M61 10H89V14.5C85 14.5 83 16.5 83 20.5V79.5C83 83.5 85 85.5 89 85.5V90H61V85.5C65 85.5 67 83.5 67 79.5V20.5C67 16.5 65 14.5 61 14.5Z" />
        <circle cx="50" cy="38.5" r="7.75" className={listening ? "mark-breathe" : undefined} />
        <circle cx="50" cy="61.5" r="7.75" className={listening ? "mark-breathe mark-breathe-late" : undefined} />
      </g>
    </svg>
  );
}
