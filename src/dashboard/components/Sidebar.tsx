import { useState } from "react";
import { useT } from "../../lib/i18n";
import { ArchivePanel } from "./ArchivePanel";
import { PlanPanel } from "./PlanPanel";
import { SongsPanel } from "./SongsPanel";

const TABS = ["Plan", "Songs", "Archive"] as const;
type Tab = (typeof TABS)[number];

function savedTab(): Tab {
  try {
    const t = localStorage.getItem("sb.tab");
    return TABS.includes(t as Tab) ? (t as Tab) : "Plan";
  } catch {
    return "Plan";
  }
}

/** The service around the sermon: the plan, songs and past services. */
export function Sidebar() {
  const [tab, setTab] = useState<Tab>(savedTab);
  const tr = useT();
  const choose = (t: Tab) => {
    setTab(t);
    try {
      localStorage.setItem("sb.tab", t);
    } catch {
      /* per-device convenience only */
    }
  };

  return (
    <section data-tour="sidebar" className="flex min-h-0 min-w-0 flex-col overflow-hidden rounded-xl border border-zinc-800 bg-white/[.02]">
      <nav data-tour="tabs" className="flex h-11 flex-none gap-5 border-b border-zinc-800 px-4 text-sm">
        {TABS.map((t) => (
          <button
            key={t}
            data-tour={`tab-${t.toLowerCase()}`}
            onClick={() => choose(t)}
            className={`border-b-2 ${t === tab ? "border-amber-500 font-medium text-zinc-100" : "border-transparent font-medium text-zinc-500 hover:text-zinc-300"}`}
          >
            {tr(t)}
          </button>
        ))}
      </nav>
      <div className="flex min-h-0 flex-1 flex-col">
        {tab === "Plan" && <PlanPanel />}
        {tab === "Songs" && <SongsPanel />}
        {tab === "Archive" && <ArchivePanel />}
      </div>
    </section>
  );
}
