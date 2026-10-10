#!/usr/bin/env python3
"""Build the read-only, FTS5-indexed scripture database bundled with Hearken Studio.

Usage:
  python3 scripts/build_bible_db.py --sample --out src-tauri/resources/bible.db
  python3 scripts/build_bible_db.py --source data/translations --out src-tauri/resources/bible.db

--source expects one CSV per translation named <CODE>.csv (e.g. KJV.csv, WEB.csv) with a header row.
Columns are detected by name (case-insensitive):
  book    : book | b | book_id | book_name   (1-66, or a name such as "1 Samuel", "I Samuel", "Psalms")
  chapter : chapter | c
  verse   : verse | v
  text    : text | t
An optional <CODE>.json next to it may contain {"name": "...", "license": "..."}.

Only bundle translations you are licensed to redistribute. KJV, WEB, ASV, YLT and BBE are public
domain; NIV, ESV, NKJV, NLT etc. require a publisher licence.
The first translation (alphabetical, or KJV if present) defines versification for validation.
"""
import argparse
import csv
import json
import re
import sqlite3
import sys
import time
from pathlib import Path

BOOKS = [
    "Genesis", "Exodus", "Leviticus", "Numbers", "Deuteronomy", "Joshua", "Judges", "Ruth", "1 Samuel",
    "2 Samuel", "1 Kings", "2 Kings", "1 Chronicles", "2 Chronicles", "Ezra", "Nehemiah", "Esther", "Job",
    "Psalms", "Proverbs", "Ecclesiastes", "Song of Solomon", "Isaiah", "Jeremiah", "Lamentations", "Ezekiel",
    "Daniel", "Hosea", "Joel", "Amos", "Obadiah", "Jonah", "Micah", "Nahum", "Habakkuk", "Zephaniah", "Haggai",
    "Zechariah", "Malachi", "Matthew", "Mark", "Luke", "John", "Acts", "Romans", "1 Corinthians",
    "2 Corinthians", "Galatians", "Ephesians", "Philippians", "Colossians", "1 Thessalonians",
    "2 Thessalonians", "1 Timothy", "2 Timothy", "Titus", "Philemon", "Hebrews", "James", "1 Peter", "2 Peter",
    "1 John", "2 John", "3 John", "Jude", "Revelation",
]

EXTRA_NAMES = {
    "psalm": 19, "song of songs": 22, "canticles": 22, "revelation of john": 66, "revelations": 66,
    "acts of the apostles": 44,
}

SCHEMA = """
PRAGMA journal_mode = OFF;
PRAGMA synchronous = OFF;
CREATE TABLE books (
  id        INTEGER PRIMARY KEY,
  name      TEXT NOT NULL,
  testament TEXT NOT NULL CHECK (testament IN ('OT','NT'))
);
CREATE TABLE translations (
  id      INTEGER PRIMARY KEY,
  code    TEXT NOT NULL UNIQUE,
  name    TEXT NOT NULL,
  license TEXT
);
CREATE TABLE verses (
  id             INTEGER PRIMARY KEY,           -- rowid, used as FTS5 content_rowid
  translation_id INTEGER NOT NULL REFERENCES translations(id),
  book_id        INTEGER NOT NULL REFERENCES books(id),
  chapter        INTEGER NOT NULL,
  verse          INTEGER NOT NULL,
  text           TEXT NOT NULL,
  UNIQUE (translation_id, book_id, chapter, verse)   -- the direct-lookup index
);
-- External-content FTS5: the index stores tokens only; text lives once, in `verses`.
CREATE VIRTUAL TABLE verses_fts USING fts5(
  text,
  content = 'verses',
  content_rowid = 'id',
  tokenize = 'porter unicode61 remove_diacritics 2'
);
"""

SAMPLE = {
    "KJV": ("King James Version", "Public Domain", [
        (1, 1, 1, "In the beginning God created the heaven and the earth."),
        (19, 23, 1, "The LORD is my shepherd; I shall not want."),
        (19, 23, 2, "He maketh me to lie down in green pastures: he leadeth me beside the still waters."),
        (43, 3, 16, "For God so loved the world, that he gave his only begotten Son, that whosoever believeth in him should not perish, but have everlasting life."),
        (43, 3, 17, "For God sent not his Son into the world to condemn the world; but that the world through him might be saved."),
        (45, 8, 28, "And we know that all things work together for good to them that love God, to them who are the called according to his purpose."),
        (46, 13, 4, "Charity suffereth long, and is kind; charity envieth not; charity vaunteth not itself, is not puffed up,"),
        (50, 4, 13, "I can do all things through Christ which strengtheneth me."),
    ]),
    "WEB": ("World English Bible", "Public Domain", [
        (1, 1, 1, "In the beginning, God created the heavens and the earth."),
        (19, 23, 1, "Yahweh is my shepherd: I shall lack nothing."),
        (19, 23, 2, "He makes me lie down in green pastures. He leads me beside still waters."),
        (43, 3, 16, "For God so loved the world, that he gave his one and only Son, that whoever believes in him should not perish, but have eternal life."),
        (43, 3, 17, "For God didn't send his Son into the world to judge the world, but that the world should be saved through him."),
        (45, 8, 28, "We know that all things work together for good for those who love God, for those who are called according to his purpose."),
        (46, 13, 4, "Love is patient and is kind. Love doesn't envy. Love doesn't brag, is not proud,"),
        (50, 4, 13, "I can do all things through Christ, who strengthens me."),
    ]),
}


def book_lookup():
    table = {}
    for i, name in enumerate(BOOKS, start=1):
        table[name.lower()] = i
        m = re.match(r"([123]) (.+)", name)
        if m:  # "I Samuel", "First Samuel", "1Samuel"
            n, base = m.groups()
            roman = {"1": "i", "2": "ii", "3": "iii"}[n]
            word = {"1": "first", "2": "second", "3": "third"}[n]
            for prefix in (roman + " ", word + " ", n):
                table[f"{prefix}{base}".lower()] = i
    table.update(EXTRA_NAMES)
    return table


def parse_book(value, lookup):
    value = value.strip()
    if value.isdigit():
        n = int(value)
        if 1 <= n <= 66:
            return n
    key = re.sub(r"\s+", " ", value.lower())
    if key in lookup:
        return lookup[key]
    raise ValueError(f"unrecognised book '{value}'")


def pick(header, *names):
    for i, h in enumerate(header):
        if h.strip().lower() in names:
            return i
    raise ValueError(f"CSV header {header} is missing one of {names}")


def read_csv(path, lookup):
    with open(path, newline="", encoding="utf-8-sig") as f:
        reader = csv.reader(f)
        header = next(reader)
        bi = pick(header, "book", "b", "book_id", "book_name")
        ci = pick(header, "chapter", "c")
        vi = pick(header, "verse", "v")
        ti = pick(header, "text", "t")
        for row in reader:
            if not row:
                continue
            text = re.sub(r"\s+", " ", row[ti]).strip()
            if text:
                yield parse_book(row[bi], lookup), int(row[ci]), int(row[vi]), text


def load_sources(args):
    if args.sample:
        for code, (name, lic, rows) in SAMPLE.items():
            yield code, name, lic, rows
        return
    src = Path(args.source)
    files = sorted(src.glob("*.csv"), key=lambda p: (p.stem.upper() != "KJV", p.stem.upper()))
    if not files:
        sys.exit(f"no CSV files in {src}")
    lookup = book_lookup()
    for path in files:
        code = path.stem.upper()
        meta_path = path.with_suffix(".json")
        meta = json.loads(meta_path.read_text()) if meta_path.exists() else {}
        yield code, meta.get("name", code), meta.get("license"), read_csv(path, lookup)


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--source", help="directory of <CODE>.csv files")
    ap.add_argument("--sample", action="store_true", help="tiny KJV+WEB sample for development")
    ap.add_argument("--out", required=True)
    args = ap.parse_args()
    if not args.sample and not args.source:
        ap.error("pass --source DIR or --sample")

    out = Path(args.out)
    out.parent.mkdir(parents=True, exist_ok=True)
    tmp = out.with_suffix(".building")
    tmp.unlink(missing_ok=True)

    db = sqlite3.connect(tmp)
    try:
        db.execute("SELECT fts5(?1)", ("x",))
    except sqlite3.OperationalError as e:
        if "no such function" in str(e) and "fts5" in str(e):
            sys.exit("This Python's SQLite lacks FTS5; use a newer Python build.")
    db.executescript(SCHEMA)
    db.executemany(
        "INSERT INTO books (id, name, testament) VALUES (?, ?, ?)",
        [(i, n, "OT" if i <= 39 else "NT") for i, n in enumerate(BOOKS, start=1)],
    )

    t0 = time.time()
    for tid, (code, name, lic, rows) in enumerate(load_sources(args), start=1):
        db.execute("INSERT INTO translations (id, code, name, license) VALUES (?, ?, ?, ?)", (tid, code, name, lic))
        cur = db.executemany(
            "INSERT INTO verses (translation_id, book_id, chapter, verse, text) VALUES (?, ?, ?, ?, ?)",
            ((tid, b, c, v, t) for b, c, v, t in rows),
        )
        print(f"  {code:6} {cur.rowcount:>7} verses")

    db.execute("INSERT INTO verses_fts (verses_fts) VALUES ('rebuild')")
    db.execute("INSERT INTO verses_fts (verses_fts) VALUES ('optimize')")
    db.execute("ANALYZE")
    db.commit()
    db.execute("VACUUM")

    # Sanity check + latency report.
    q0 = time.perf_counter()
    hits = db.execute(
        "SELECT v.book_id, v.chapter, v.verse FROM verses_fts JOIN verses v ON v.id = verses_fts.rowid "
        "WHERE verses_fts MATCH ? ORDER BY bm25(verses_fts) LIMIT 5",
        ('"loved" "world"',),
    ).fetchall()
    q_ms = (time.perf_counter() - q0) * 1000
    db.close()
    tmp.replace(out)
    print(f"built {out} ({out.stat().st_size / 1e6:.1f} MB) in {time.time() - t0:.1f}s; "
          f"FTS probe {q_ms:.2f} ms -> {hits[:2]}")


if __name__ == "__main__":
    main()
