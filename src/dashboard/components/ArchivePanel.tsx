import { useEffect, useState } from "react";
import { api, type ArchiveHit, type DaySummary, type Insights, type RecapFormat } from "../../lib/ipc";
import { useDashboard } from "../store";
import { useDismiss } from "../useDismiss";

const FORMATS: { format: RecapFormat; label: string }[] = [
  { format: "pdf", label: "PDF" },
  { format: "docx", label: "Word (.docx)" },
  { format: "doc", label: "Word 97–2003 (.doc)" },
  { format: "html", label: "Web page" },
];

const KIND = { said: "Said", shown: "Shown", mentioned: "Mentioned" } as const;

function longDate(iso: string) {
  const d = new Date(`${iso}T12:00:00`);
  return Number.isNaN(d.getTime()) ? iso : d.toLocaleDateString(undefined, { weekday: "short", day: "numeric", month: "short", year: "numeric" });
}

/** Past services: shareable recaps, a searchable sermon archive, and yearly insights. */
export function ArchivePanel() {
  const [sessions, setSessions] = useState<DaySummary[]>([]);
  const [notice, setNotice] = useState<{ text: string; error?: boolean } | null>(null);

  useEffect(() => {
    api.listSessions().then(setSessions).catch((e) => setNotice({ text: String(e), error: true }));
  }, []);

  const recap = async (date: string, format: RecapFormat) => {
    try {
      setNotice({ text: `Recap saved to ${await api.exportRecap(date, format)}` });
    } catch (e) {
      setNotice({ text: String(e), error: true });
    }
  };

  return (
    <div className="min-h-0 flex-1 space-y-5 overflow-y-auto p-3">
      <ArchiveSearch />

      <section className="space-y-2">
        <h3 className="text-xs font-semibold uppercase tracking-wider text-zinc-400">Services</h3>
        <ChurchName />
        {notice && <p className={`break-all text-xs ${notice.error ? "text-red-400" : "text-emerald-400"}`}>{notice.text}</p>}
        {sessions.length === 0 && <p className="text-sm text-zinc-500">Services appear here after you listen or show slides.</p>}
        <ul className="space-y-1">
          {sessions.map((s) => (
            <li key={s.date} className="flex items-center gap-2 rounded-md px-2 py-1.5 hover:bg-zinc-900">
              <div className="min-w-0 flex-1">
                <div className="text-sm font-medium">{longDate(s.date)}</div>
                <div className="text-xs text-zinc-500">
                  {[s.firstAt && s.lastAt && `${s.firstAt}–${s.lastAt}`, `${s.scriptures} scriptures`, s.songs ? `${s.songs} songs` : null, `${s.words.toLocaleString()} words`]
                    .filter(Boolean)
                    .join(" · ")}
                </div>
              </div>
              <RecapMenu onPick={(f) => recap(s.date, f)} />
            </li>
          ))}
        </ul>
      </section>

      <InsightsView />
    </div>
  );
}

/** Save a service's recap (scriptures, sermon notes, songs) in the chosen format. */
function RecapMenu({ onPick }: { onPick: (format: RecapFormat) => void }) {
  const [open, setOpen] = useState(false);
  const ref = useDismiss<HTMLDivElement>(open, () => setOpen(false));
  return (
    <div ref={ref} className="relative flex-none">
      <button onClick={() => setOpen(!open)} className="rounded bg-zinc-800 px-2 py-1 text-xs hover:bg-zinc-700" title="Save the scriptures, sermon notes and songs to share">
        Recap ▾
      </button>
      {open && (
        <ul className="absolute right-0 top-full z-20 mt-1 w-44 rounded-md bg-zinc-900 py-1 shadow-xl ring-1 ring-zinc-700">
          {FORMATS.map(({ format, label }) => (
            <li key={format}>
              <button
                onClick={() => {
                  setOpen(false);
                  onPick(format);
                }}
                className="w-full px-3 py-1.5 text-left text-xs hover:bg-zinc-800"
              >
                {label}
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

function ChurchName() {
  const name = useDashboard((s) => s.settings?.churchName ?? "");
  const saveSettings = useDashboard((s) => s.saveSettings);
  const [value, setValue] = useState(name);
  useEffect(() => {
    setValue(name);
  }, [name]);
  return (
    <label className="flex items-center gap-2 text-xs text-zinc-500">
      Recap heading
      <input
        value={value}
        onChange={(e) => setValue(e.target.value)}
        onBlur={() => value !== name && void saveSettings({ churchName: value })}
        placeholder="Your church's name"
        className="min-w-0 flex-1 rounded bg-zinc-900 px-2 py-1 text-sm text-zinc-100 outline-none ring-1 ring-zinc-800 focus:ring-amber-500"
      />
    </label>
  );
}

function ArchiveSearch() {
  const translation = useDashboard((s) => s.settings?.translation ?? "KJV");
  const setPreview = useDashboard((s) => s.setPreview);
  const [query, setQuery] = useState("");
  const [hits, setHits] = useState<ArchiveHit[] | null>(null);

  useEffect(() => {
    if (!query.trim()) {
      setHits(null);
      return;
    }
    const id = setTimeout(() => api.searchArchive(query).then(setHits).catch(() => setHits([])), 200);
    return () => clearTimeout(id);
  }, [query]);

  const preview = async (label: string) => {
    const r = await api.searchScripture(label, translation);
    if (r.reference) setPreview(r.reference);
  };

  return (
    <section className="space-y-2">
      <input
        value={query}
        onChange={(e) => setQuery(e.target.value)}
        placeholder="Search past sermons: “Romans 8” or “mustard seed”"
        className="w-full rounded-md bg-zinc-900 px-3 py-2 text-sm outline-none ring-1 ring-zinc-800 focus:ring-amber-500"
      />
      {hits && (
        <ul className="space-y-1.5">
          {hits.length === 0 && <li className="text-sm text-zinc-500">Nothing found.</li>}
          {hits.map((h, i) => (
            <li
              key={i}
              onClick={h.label ? () => void preview(h.label!) : undefined}
              className={`rounded-md bg-zinc-900 px-2.5 py-2 text-sm ${h.label ? "cursor-pointer hover:bg-zinc-800" : ""}`}
            >
              <div className="flex gap-2 text-xs text-zinc-500">
                <span>{longDate(h.date)}</span>
                <span>{h.time}</span>
                <span className="text-amber-500">{KIND[h.kind]}</span>
              </div>
              {h.label && <div className="font-semibold">{h.label}</div>}
              <p className="line-clamp-2 text-zinc-400">{h.text}</p>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}

function InsightsView() {
  const [year, setYear] = useState(new Date().getFullYear());
  const [data, setData] = useState<Insights | null>(null);
  useEffect(() => {
    api.getInsights(year).then(setData).catch(() => setData(null));
  }, [year]);
  if (!data) return null;

  return (
    <section className="space-y-3">
      <div className="flex items-center justify-between">
        <h3 className="text-xs font-semibold uppercase tracking-wider text-zinc-400">Insights</h3>
        <div className="flex items-center gap-1 text-sm">
          <button onClick={() => setYear(year - 1)} className="rounded px-1.5 text-zinc-400 hover:bg-zinc-800" aria-label="Previous year">
            ‹
          </button>
          <span className="font-mono">{year}</span>
          <button onClick={() => setYear(year + 1)} className="rounded px-1.5 text-zinc-400 hover:bg-zinc-800" aria-label="Next year">
            ›
          </button>
        </div>
      </div>
      <div className="grid grid-cols-3 gap-2">
        <Stat label="Services" value={data.services} />
        <Stat label="Scriptures shown" value={data.scripturesShown} />
        <Stat
          label="Old / New Testament"
          value={data.scripturesShown ? `${Math.round((data.oldTestament / data.scripturesShown) * 100)}% / ${Math.round((data.newTestament / data.scripturesShown) * 100)}%` : "—"}
        />
      </div>
      {data.services === 0 ? (
        <p className="text-sm text-zinc-500">No services recorded in {year}.</p>
      ) : (
        <>
          <Bars title="Books preached most" rows={data.books} />
          <Bars title="Most shown passages" rows={data.verses} />
          {data.songs.length > 0 && <Bars title="Most sung songs" rows={data.songs} />}
        </>
      )}
    </section>
  );
}

function Stat({ label, value }: { label: string; value: string | number }) {
  return (
    <div className="rounded-md bg-zinc-900 px-2.5 py-2">
      <div className="text-lg font-semibold tabular-nums">{value}</div>
      <div className="text-[11px] leading-tight text-zinc-500">{label}</div>
    </div>
  );
}

function Bars({ title, rows }: { title: string; rows: [string, number][] }) {
  const max = Math.max(1, ...rows.map((r) => r[1]));
  return (
    <div>
      <h4 className="mb-1.5 text-xs text-zinc-400">{title}</h4>
      <ul className="space-y-1">
        {rows.map(([name, n]) => (
          <li key={name} className="grid grid-cols-[minmax(0,9rem)_1fr_2rem] items-center gap-2 text-xs">
            <span className="truncate text-zinc-300" title={name}>
              {name}
            </span>
            <span className="h-2 rounded-sm bg-zinc-800">
              <span className="block h-full rounded-sm bg-amber-500" style={{ width: `${(n / max) * 100}%` }} />
            </span>
            <span className="text-right tabular-nums text-zinc-400">{n}</span>
          </li>
        ))}
      </ul>
    </div>
  );
}
