import { useState } from "react";
import { ArchivePanel } from "./ArchivePanel";
import { PlanPanel } from "./PlanPanel";
import { SongsPanel } from "./SongsPanel";
import { TranscriptPanel } from "./TranscriptPanel";

const TABS = ["Transcript", "Plan", "Songs", "Archive"] as const;
type Tab = (typeof TABS)[number];

function savedTab(): Tab {
  try {
    const t = localStorage.getItem("sb.tab");
    return TABS.includes(t as Tab) ? (t as Tab) : "Transcript";
  } catch {
    return "Transcript";
  }
}

export function Sidebar() {
  const [tab, setTab] = useState<Tab>(savedTab);
  const choose = (t: Tab) => {
    setTab(t);
    try {
      localStorage.setItem("sb.tab", t);
    } catch {
      /* per-device convenience only */
    }
  };

  return (
    <section className="flex min-h-0 min-w-0 flex-col overflow-x-hidden bg-zinc-950">
      <nav className="flex h-10 flex-none border-b border-zinc-800 px-2 text-xs font-semibold uppercase tracking-wider">
        {TABS.map((t) => (
          <button
            key={t}
            onClick={() => choose(t)}
            className={`border-b-2 px-3 ${t === tab ? "border-amber-500 text-zinc-100" : "border-transparent text-zinc-500 hover:text-zinc-300"}`}
          >
            {t}
          </button>
        ))}
      </nav>
      <div className="flex min-h-0 flex-1 flex-col">
        {tab === "Transcript" && <TranscriptPanel />}
        {tab === "Plan" && <PlanPanel />}
        {tab === "Songs" && <SongsPanel />}
        {tab === "Archive" && <ArchivePanel />}
      </div>
    </section>
  );
}
