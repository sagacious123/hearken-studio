import { useEffect, useMemo, useRef, useState } from "react";
import { type T, useT } from "../../lib/i18n";
import { api, type Slide, type SongSummary } from "../../lib/ipc";
import { useDashboard } from "../store";
import { SlideRow } from "./SlideRow";

// The section headers stay in English: they are what the song parser recognises.
const template = (t: T) => `${t("Song title")}
CCLI Song # 1234567

Verse 1
${t("First line of the verse")}
${t("Second line")}

Chorus
${t("First line of the chorus")}
${t("Second line")}`;

/** The parser's own name for a part with no header ("Part 3"); headers the user typed stay as they are. */
const sectionTitle = (label: string | undefined, t: T) => {
  const m = label?.match(/^Part (\d+)( \(\d+\))?$/);
  return m ? t("Part {n}", { n: m[1] }) + (m[2] ?? "") : label;
};

type View = { kind: "list" } | { kind: "song"; id: string } | { kind: "edit"; id: string | null; text: string };

/**
 * Worship-song library. Nothing is bundled: churches import the lyrics they're licensed to
 * project. While listening, the app recognises the song being sung and follows it.
 */
export function SongsPanel() {
  const t = useT();
  const lyricsOn = useDashboard((s) => s.settings?.lyricsDetection ?? true);
  const [songs, setSongs] = useState<SongSummary[]>([]);
  const [view, setView] = useState<View>({ kind: "list" });
  const [filter, setFilter] = useState("");
  const [notice, setNotice] = useState<{ text: string; error?: boolean } | null>(null);
  const fileRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    api.listSongs().then(setSongs).catch((e) => setNotice({ text: String(e), error: true }));
  }, []);

  const shown = useMemo(() => {
    const q = filter.trim().toLowerCase();
    return q ? songs.filter((s) => s.title.toLowerCase().includes(q) || s.author?.toLowerCase().includes(q)) : songs;
  }, [songs, filter]);

  const importFiles = async (files: FileList) => {
    const failed: string[] = [];
    let latest = songs;
    for (const f of Array.from(files)) {
      try {
        latest = await api.saveSong(await f.text(), null);
      } catch (e) {
        failed.push(`${f.name}: ${String(e)}`);
      }
    }
    setSongs(latest);
    const added = files.length - failed.length;
    setNotice(
      failed.length
        ? { text: t("Imported {added} of {total}. {errors}", { added, total: files.length, errors: failed.join("; ") }), error: true }
        : { text: added === 1 ? t("Imported 1 song.") : t("Imported {n} songs.", { n: added }) },
    );
  };

  if (view.kind === "edit") {
    return (
      <SongEditor
        view={view}
        onCancel={() => setView(view.id ? { kind: "song", id: view.id } : { kind: "list" })}
        onSaved={(list, id) => {
          setSongs(list);
          setView(id ? { kind: "song", id } : { kind: "list" });
        }}
      />
    );
  }

  if (view.kind === "song") {
    const song = songs.find((s) => s.id === view.id);
    if (!song) return null;
    return (
      <SongDetail
        song={song}
        onBack={() => setView({ kind: "list" })}
        onEdit={async () => setView({ kind: "edit", id: song.id, text: await api.getSongText(song.id) })}
        onDelete={async () => {
          if (!window.confirm(t("Delete “{title}” from the library?", { title: song.title }))) return;
          setSongs(await api.deleteSong(song.id));
          setView({ kind: "list" });
        }}
      />
    );
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="flex-none space-y-2 border-b border-zinc-800 p-3">
        <div data-tour="songs-add" className="flex gap-2">
          <input
            value={filter}
            onChange={(e) => setFilter(e.target.value)}
            placeholder={songs.length === 1 ? t("Search 1 song") : t("Search {n} songs", { n: songs.length })}
            className="min-w-0 flex-1 rounded-md bg-zinc-900 px-2 py-1 text-sm outline-none ring-1 ring-zinc-800 focus:ring-amber-500"
          />
          <button onClick={() => setView({ kind: "edit", id: null, text: "" })} className="rounded bg-amber-600 px-2.5 py-1 text-sm font-medium hover:bg-amber-500">
            {t("+ Add")}
          </button>
          <button onClick={() => fileRef.current?.click()} className="rounded bg-zinc-800 px-2.5 py-1 text-sm hover:bg-zinc-700">
            {t("Import")}
          </button>
          <input
            ref={fileRef}
            type="file"
            multiple
            accept=".txt,text/plain"
            className="hidden"
            onChange={(e) => {
              if (e.target.files?.length) void importFiles(e.target.files);
              e.target.value = "";
            }}
          />
        </div>
        <p className="text-xs text-zinc-500">
          {t("Hearken Studio includes no lyrics. Add the songs your church is licensed to project (e.g. paste from CCLI SongSelect, or import .txt files).")}
          {!lyricsOn && ` ${t("Song recognition is off: turn on “Follow worship songs” in Settings.")}`}
        </p>
        {notice && <p className={`text-xs ${notice.error ? "text-red-400" : "text-emerald-400"}`}>{notice.text}</p>}
      </div>
      <ul className="min-h-0 flex-1 overflow-y-auto p-2">
        {songs.length === 0 && <li className="p-2 text-sm text-zinc-500">{t("No songs yet. Add one to have its lyrics follow the singing.")}</li>}
        {shown.map((s) => (
          <li key={s.id}>
            <button onClick={() => setView({ kind: "song", id: s.id })} className="w-full rounded-md px-2 py-1.5 text-left hover:bg-zinc-900">
              <span className="font-medium">{s.title}</span>
              <span className="ml-2 text-xs text-zinc-500">
                {s.sections.length === 1 ? t("1 part") : t("{n} parts", { n: s.sections.length })}{s.ccli ? ` · CCLI ${s.ccli}` : ""}
              </span>
            </button>
          </li>
        ))}
      </ul>
      <div className="flex-none border-t border-zinc-800 p-2">
        <button
          onClick={async () => {
            try {
              setNotice({ text: t("Saved {path}", { path: await api.exportCcliReport(90) }) });
            } catch (e) {
              setNotice({ text: String(e), error: true });
            }
          }}
          data-tour="songs-ccli"
          className="text-xs text-zinc-400 hover:text-zinc-200"
        >
          {t("Export song usage for CCLI (last 90 days)")}
        </button>
      </div>
    </div>
  );
}

function SongDetail({ song, onBack, onEdit, onDelete }: { song: SongSummary; onBack: () => void; onEdit: () => void; onDelete: () => void }) {
  const t = useT();
  const [slides, setSlides] = useState<Slide[]>([]);
  useEffect(() => {
    Promise.all(song.sections.map((_, i) => api.songSlide(song.id, i))).then((all) => setSlides(all.filter((s): s is Slide => !!s)));
  }, [song]);

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="flex flex-none items-center gap-2 border-b border-zinc-800 p-3">
        <button onClick={onBack} className="rounded px-1.5 text-zinc-400 hover:bg-zinc-800 hover:text-zinc-100" aria-label={t("Back to songs")}>
          ←
        </button>
        <div className="min-w-0 flex-1">
          <div className="truncate font-semibold">{song.title}</div>
          <div className="text-xs text-zinc-500">
            {[song.author, song.ccli && `CCLI ${song.ccli}`].filter(Boolean).join(" · ") || t("No CCLI number")}
          </div>
        </div>
        <button onClick={onEdit} className="rounded bg-zinc-800 px-2 py-1 text-xs hover:bg-zinc-700">
          {t("Edit")}
        </button>
        <button onClick={onDelete} className="rounded px-2 py-1 text-xs text-red-400 hover:bg-red-950">
          {t("Delete")}
        </button>
      </div>
      <ul className="min-h-0 flex-1 space-y-2 overflow-y-auto p-3">
        {slides.map((s) => (
          <SlideRow key={s.id} slide={s} title={sectionTitle(song.sections[s.song?.section ?? 0], t)} />
        ))}
      </ul>
    </div>
  );
}

function SongEditor({
  view,
  onCancel,
  onSaved,
}: {
  view: { id: string | null; text: string };
  onCancel: () => void;
  onSaved: (songs: SongSummary[], id: string | null) => void;
}) {
  const t = useT();
  const [text, setText] = useState(view.text);
  const [error, setError] = useState<string | null>(null);
  const save = async () => {
    try {
      const list = await api.saveSong(text, view.id);
      const firstLine = text.trim().split("\n")[0]?.trim().toLowerCase();
      onSaved(list, view.id ?? list.find((s) => s.title.toLowerCase() === firstLine)?.id ?? null);
    } catch (e) {
      setError(String(e));
    }
  };
  return (
    <div className="flex min-h-0 flex-1 flex-col gap-2 p-3">
      <p className="text-xs text-zinc-500">
        {t("First line is the title. Separate parts with a blank line; start a part with “Verse 1”, “Chorus”, “Bridge”… Copyright footer lines are removed.")}
      </p>
      <textarea
        value={text}
        onChange={(e) => setText(e.target.value)}
        placeholder={template(t)}
        spellCheck={false}
        autoFocus
        className="min-h-0 flex-1 resize-none rounded-md bg-zinc-900 p-2 font-mono text-sm outline-none ring-1 ring-zinc-800 focus:ring-amber-500"
      />
      {error && <p className="text-xs text-red-400">{error}</p>}
      <div className="flex gap-2">
        <button onClick={save} className="rounded bg-amber-600 px-3 py-1 text-sm font-medium hover:bg-amber-500">
          {t("Save song")}
        </button>
        <button onClick={onCancel} className="rounded bg-zinc-800 px-3 py-1 text-sm hover:bg-zinc-700">
          {t("Cancel")}
        </button>
      </div>
    </div>
  );
}
