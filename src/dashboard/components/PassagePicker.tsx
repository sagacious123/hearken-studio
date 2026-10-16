import { useEffect, useId, useMemo, useRef, useState, type FocusEvent, type KeyboardEvent, type MouseEvent } from "react";
import { useT, useUiLanguage, type UiLanguage } from "../../lib/i18n";
import { api, type BookInfo, type Slide } from "../../lib/ipc";
import { useDashboard } from "../store";

/** Most verses on one slide (matches the Rust side); longer ranges are stepped with ▶. */
const MAX_SLIDE_VERSES = 6;

/** Lowercase without accents: "Genèse" → "genese". */
const fold = (s: string) => s.normalize("NFD").replace(/\p{M}/gu, "").toLowerCase();

const norm = (s: string) =>
  fold(s)
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

/** Waiting for a second Enter on exactly the passage the first one previewed. */
const armedFor = (armed: string | null, key: string) => armed !== null && armed === key;

/** Focus (and select) a field after React has rendered, e.g. once it's no longer disabled. */
const focusSoon = (el: HTMLInputElement | null) => requestAnimationFrame(() => el?.focus());

/** A book's name in the interface language (English when there's none). */
const bookName = (b: BookInfo, lang: UiLanguage) => (lang === "en" ? b.name : (b.names[lang] ?? b.name));

/** Books matching what's typed: name or alias prefix first, then anywhere, in Bible order. */
function filterBooks(books: BookInfo[], query: string): BookInfo[] {
  const q = norm(query);
  if (!q) return books;
  const scored = books
    .map((b) => {
      const names = [b.name, ...Object.values(b.names)].map(fold);
      const score = names.some((n) => n.startsWith(q)) ? 0 : b.aliases.some((a) => a.startsWith(q)) ? 1 : names.some((n) => n.includes(q)) ? 2 : b.aliases.some((a) => a.includes(q)) ? 3 : 9;
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
  const t = useT();
  const lang = useUiLanguage();
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
      ? chapters === 1
        ? t("{book} has 1 chapter", { book: bookName(book, lang) })
        : t("{book} has {n} chapters", { book: bookName(book, lang), n: chapters })
      : from && (vFrom < 1 || vFrom > maxVerse || !Number.isInteger(vFrom))
        ? t("{book} {chapter} has {n} verses", { book: bookName(book, lang), chapter: ch, n: maxVerse })
        : to && (vTo < vFrom || vTo > maxVerse || !Number.isInteger(vTo))
          ? vTo < vFrom
            ? t("The last verse comes before the first")
            : t("{book} {chapter} has {n} verses", { book: bookName(book, lang), chapter: ch, n: maxVerse })
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

  // Enter previews; Enter again on the same passage (nothing changed) takes it live.
  const passageKey = `${book?.id}:${chapter}:${from}:${to}:${translation}`;
  const [armed, setArmed] = useState<string | null>(null);
  const again = armedFor(armed, passageKey);
  const onEnter = (e: KeyboardEvent<HTMLInputElement>, next?: () => void) => {
    if (e.key !== "Enter") return;
    e.preventDefault();
    if (e.metaKey || e.ctrlKey || again) {
      void submit(true);
      setArmed(null);
    } else if (next) next();
    else if (!problem && book && chapter) {
      void submit(false);
      setArmed(passageKey);
    }
  };

  const num = "h-10 w-12 rounded-lg bg-white/[.02] px-2 text-sm tabular-nums outline-none ring-1 ring-zinc-800 focus:ring-amber-500 [appearance:textfield] [&::-webkit-inner-spin-button]:appearance-none";
  const rangeLong = from && to && vTo - vFrom + 1 > MAX_SLIDE_VERSES;

  return (
    <div className="space-y-1.5">
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
          placeholder={t("Ch.")}
          aria-label={t("Chapter")}
          title={book ? `1–${chapters}` : t("Pick a book first")}
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
          placeholder={t("V.")}
          aria-label={t("First verse")}
          title={maxVerse ? `1–${maxVerse}` : t("Whole chapter if empty")}
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
          placeholder={t("to")}
          aria-label={t("Last verse (optional)")}
          disabled={!from}
          className={num}
        />
      </div>
      {(problem || rangeLong || again) && (
        <p className={`text-xs ${problem ? "text-red-400" : again ? "text-amber-300" : "text-zinc-500"}`}>
          {problem ?? (again ? t("Press Enter again to go live") : t("Shows {n} verses at a time; use Verse ▶ for the rest.", { n: MAX_SLIDE_VERSES }))}
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
  const t = useT();
  const lang = useUiLanguage();

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
        aria-label={t("Book")}
        value={open ? query : (value ? bookName(value, lang) : "")}
        placeholder={t("Book")}
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
        className="h-10 w-full rounded-lg bg-white/[.02] px-3 text-sm outline-none ring-1 ring-zinc-800 placeholder:text-zinc-500 focus:ring-amber-500"
      />
      {open && (
        <ul ref={listRef} id={listId} role="listbox" className="absolute inset-x-0 top-full z-20 mt-1 max-h-72 overflow-y-auto rounded-md bg-zinc-900 py-1 shadow-xl ring-1 ring-zinc-700">
          {matches.length === 0 && <li className="px-3 py-1.5 text-sm text-zinc-500">{t("No book matches “{query}”", { query })}</li>}
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
              {bookName(b, lang)}
              <span className={`text-xs ${i === active ? "text-amber-100" : "text-zinc-500"}`}>{t("{n} ch.", { n: b.verses.length })}</span>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
