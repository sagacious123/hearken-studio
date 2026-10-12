import { useEffect, useId, useMemo, useRef, useState, type FocusEvent, type KeyboardEvent, type MouseEvent } from "react";
import { api, type BookInfo, type Slide } from "../../lib/ipc";
import { useDashboard } from "../store";

/** Most verses on one slide (matches the Rust side); longer ranges are stepped with ▶. */
const MAX_SLIDE_VERSES = 6;

const norm = (s: string) =>
  s
    .toLowerCase()
    .replace(/^([123])(?=[a-z])/, "$1 ") // "1cor" → "1 cor"
    .replace(/\s+/g, " ")
    .trim();

/**
 * Select a field's whole value when it gains focus, so typing replaces it. A click focuses on
 * mouse-down and the following mouse-up would collapse the selection, so that one is cancelled.
 */
function useSelectOnFocus() {
  const justFocused = useRef(false);
  return {
    onFocus: (e: FocusEvent<HTMLInputElement>) => {
      e.currentTarget.select();
      justFocused.current = true;
    },
    onMouseUp: (e: MouseEvent<HTMLInputElement>) => {
      if (justFocused.current) e.preventDefault();
      justFocused.current = false;
    },
    onBlur: () => {
      justFocused.current = false;
    },
  };
}

/** Focus (and select) a field after React has rendered, e.g. once it's no longer disabled. */
const focusSoon = (el: HTMLInputElement | null) => requestAnimationFrame(() => el?.focus());

/** Books matching what's typed: name or alias prefix first, then anywhere, in Bible order. */
function filterBooks(books: BookInfo[], query: string): BookInfo[] {
  const q = norm(query);
  if (!q) return books;
  const scored = books
    .map((b) => {
      const name = b.name.toLowerCase();
      const score = name.startsWith(q) ? 0 : b.aliases.some((a) => a.startsWith(q)) ? 1 : name.includes(q) ? 2 : b.aliases.some((a) => a.includes(q)) ? 3 : 9;
      return { b, score };
    })
    .filter((x) => x.score < 9);
  return scored.sort((x, y) => x.score - y.score || x.b.id - y.b.id).map((x) => x.b);
}

/**
 * Pick a passage by hand: searchable book list, then chapter and verse(s), checked against the
 * real chapter and verse counts. Previews as you type; Enter previews, ⌘/Ctrl+Enter goes live.
 */
export function PassagePicker() {
  const translation = useDashboard((s) => s.settings?.translation ?? "KJV");
  const setPreview = useDashboard((s) => s.setPreview);
  const goLive = useDashboard((s) => s.goLive);
  const [books, setBooks] = useState<BookInfo[]>([]);
  const [book, setBook] = useState<BookInfo | null>(null);
  const [chapter, setChapter] = useState("");
  const [from, setFrom] = useState("");
  const [to, setTo] = useState("");
  const selectOnFocus = useSelectOnFocus();
  const chapterRef = useRef<HTMLInputElement>(null);
  const fromRef = useRef<HTMLInputElement>(null);
  const toRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    api.listBooks().then(setBooks).catch(console.error);
  }, []);

  const ch = Number(chapter);
  const chapters = book?.verses.length ?? 0;
  const maxVerse = book && ch >= 1 && ch <= chapters ? book.verses[ch - 1] : 0;
  const vFrom = Number(from);
  const vTo = Number(to);

  const problem = !book
    ? null
    : chapter && (ch < 1 || ch > chapters || !Number.isInteger(ch))
      ? `${book.name} has ${chapters} chapter${chapters === 1 ? "" : "s"}`
      : from && (vFrom < 1 || vFrom > maxVerse || !Number.isInteger(vFrom))
        ? `${book.name} ${ch} has ${maxVerse} verses`
        : to && (vTo < vFrom || vTo > maxVerse || !Number.isInteger(vTo))
          ? vTo < vFrom
            ? "The last verse comes before the first"
            : `${book.name} ${ch} has ${maxVerse} verses`
          : null;
  const ready = !!book && !!chapter && !problem;

  const build = async (): Promise<Slide | null> => {
    if (!ready || !book) return null;
    const verseStart = from ? vFrom : null;
    const verseEnd = verseStart && to && vTo > verseStart ? vTo : null;
    return api.buildSlide({ bookId: book.id, chapter: ch, verseStart, verseEnd }, translation);
  };

  // Preview follows every keystroke immediately (a passage lookup takes well under a
  // millisecond); a reply that arrives after a newer keystroke is ignored.
  const latest = useRef(0);
  useEffect(() => {
    if (!ready) return;
    const request = ++latest.current;
    void build().then((s) => {
      if (s && request === latest.current) setPreview(s);
    });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [ready, book?.id, chapter, from, to, translation]);

  const submit = async (live: boolean) => {
    const slide = await build();
    if (!slide) return;
    if (live) await goLive(slide);
    else setPreview(slide);
  };

  const onEnter = (e: KeyboardEvent<HTMLInputElement>, next?: () => void) => {
    if (e.key !== "Enter") return;
    e.preventDefault();
    if (e.metaKey || e.ctrlKey) void submit(true);
    else if (next) next();
    else void submit(false);
  };

  const num = "w-12 rounded-md bg-zinc-900 px-2 py-1.5 text-sm tabular-nums outline-none ring-1 ring-zinc-800 focus:ring-amber-500 [appearance:textfield] [&::-webkit-inner-spin-button]:appearance-none";
  const rangeLong = from && to && vTo - vFrom + 1 > MAX_SLIDE_VERSES;

  return (
    <div className="flex-none space-y-1.5 border-b border-zinc-800 px-3 pb-3">
      <div className="flex flex-wrap items-center gap-1.5">
        <BookCombobox
          books={books}
          value={book}
          onPick={(b) => {
            setBook(b);
            if (b.verses.length < Number(chapter)) setChapter("");
            setFrom("");
            setTo("");
            focusSoon(chapterRef.current); // it was disabled until this book was set
          }}
        />
        <input
          ref={chapterRef}
          value={chapter}
          onChange={(e) => setChapter(e.target.value.replace(/\D/g, ""))}
          onKeyDown={(e) => onEnter(e, from ? undefined : () => focusSoon(fromRef.current))}
          {...selectOnFocus}
          inputMode="numeric"
          placeholder="Ch."
          aria-label="Chapter"
          title={book ? `1–${chapters}` : "Pick a book first"}
          disabled={!book}
          className={num}
        />
        <span className="text-zinc-600">:</span>
        <input
          ref={fromRef}
          value={from}
          onChange={(e) => setFrom(e.target.value.replace(/\D/g, ""))}
          onKeyDown={(e) => onEnter(e)}
          {...selectOnFocus}
          inputMode="numeric"
          placeholder="V."
          aria-label="First verse"
          title={maxVerse ? `1–${maxVerse}` : "Whole chapter if empty"}
          disabled={!book || !chapter}
          className={num}
        />
        <span className="text-zinc-600">–</span>
        <input
          ref={toRef}
          value={to}
          onChange={(e) => setTo(e.target.value.replace(/\D/g, ""))}
          onKeyDown={(e) => onEnter(e)}
          {...selectOnFocus}
          inputMode="numeric"
          placeholder="to"
          aria-label="Last verse (optional)"
          disabled={!from}
          className={num}
        />
        <button
          onClick={() => void submit(false)}
          disabled={!ready}
          className="rounded bg-zinc-800 px-2.5 py-1.5 text-xs font-medium hover:bg-zinc-700 disabled:opacity-40"
          title="Show in Preview (Enter)"
        >
          Preview
        </button>
        <button
          onClick={() => void submit(true)}
          disabled={!ready}
          className="rounded bg-red-600 px-2.5 py-1.5 text-xs font-semibold text-white hover:bg-red-500 disabled:opacity-40"
          title="Send to the screen (⌘/Ctrl+Enter)"
        >
          Live
        </button>
      </div>
      {(problem || rangeLong) && (
        <p className={`text-xs ${problem ? "text-red-400" : "text-zinc-500"}`}>
          {problem ?? `Shows ${MAX_SLIDE_VERSES} verses at a time; use Verse ▶ for the rest.`}
        </p>
      )}
    </div>
  );
}

function BookCombobox({ books, value, onPick }: { books: BookInfo[]; value: BookInfo | null; onPick: (b: BookInfo) => void }) {
  const listId = useId();
  const [query, setQuery] = useState("");
  const [open, setOpen] = useState(false);
  const [active, setActive] = useState(0);
  const listRef = useRef<HTMLUListElement>(null);
  const matches = useMemo(() => filterBooks(books, query), [books, query]);

  useEffect(() => {
    listRef.current?.children[active]?.scrollIntoView({ block: "nearest" });
  }, [active, open]);

  const pick = (b: BookInfo | undefined) => {
    if (!b) return;
    onPick(b);
    setQuery("");
    setOpen(false);
  };

  return (
    <div className="relative min-w-0 flex-1 basis-32">
      <input
        role="combobox"
        aria-expanded={open}
        aria-controls={listId}
        aria-activedescendant={open && matches[active] ? `${listId}-${matches[active].id}` : undefined}
        aria-label="Book"
        value={open ? query : (value?.name ?? "")}
        placeholder="Book"
        onFocus={(e) => {
          setOpen(true);
          setActive(0);
          e.target.select();
        }}
        onBlur={() => setOpen(false)}
        onChange={(e) => {
          setQuery(e.target.value);
          setActive(0);
          setOpen(true);
        }}
        onKeyDown={(e) => {
          if (e.key === "ArrowDown") {
            e.preventDefault();
            setOpen(true);
            setActive((a) => Math.min(a + 1, matches.length - 1));
          } else if (e.key === "ArrowUp") {
            e.preventDefault();
            setActive((a) => Math.max(a - 1, 0));
          } else if (e.key === "Enter") {
            e.preventDefault();
            pick(matches[active]);
          } else if (e.key === "Tab" && open && query) {
            pick(matches[active]); // Tab accepts the highlighted book, then moves on
          } else if (e.key === "Escape") {
            setOpen(false);
            setQuery("");
          }
        }}
        className="w-full rounded-md bg-zinc-900 px-2 py-1.5 text-sm outline-none ring-1 ring-zinc-800 focus:ring-amber-500"
      />
      {open && (
        <ul ref={listRef} id={listId} role="listbox" className="absolute inset-x-0 top-full z-20 mt-1 max-h-72 overflow-y-auto rounded-md bg-zinc-900 py-1 shadow-xl ring-1 ring-zinc-700">
          {matches.length === 0 && <li className="px-3 py-1.5 text-sm text-zinc-500">No book matches “{query}”</li>}
          {matches.map((b, i) => (
            <li
              key={b.id}
              id={`${listId}-${b.id}`}
              role="option"
              aria-selected={i === active}
              onMouseDown={(e) => {
                e.preventDefault(); // keep focus; pick before blur closes the list
                pick(b);
              }}
              onMouseEnter={() => setActive(i)}
              className={`flex cursor-pointer items-baseline justify-between px-3 py-1 text-sm ${i === active ? "bg-amber-600 text-white" : "text-zinc-200"}`}
            >
              {b.name}
              <span className={`text-xs ${i === active ? "text-amber-100" : "text-zinc-500"}`}>{b.verses.length} ch.</span>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
