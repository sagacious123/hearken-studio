#!/usr/bin/env python3
"""Download the five public-domain translations into data/translations/ as <CODE>.csv + <CODE>.json,
ready for build_bible_db.py. Safe to re-run; existing files are skipped (pass --force to refetch).

Sources:
  KJV, ASV, YLT, BBE : scrollmapper/bible_databases CSVs, pinned to a commit
  WEB                : eBible.org verse-per-line text (Protestant canon, "engwebp")

Light clean-ups so text projects cleanly; wording is never changed:
  KJV : en dashes in hyphenated names ("Beth–el") back to hyphens, as printed
  ASV : drop the unmatched "[" scrollmapper leaves before "Selah" / "Higgaion"
  YLT : backtick opening quotes (`Let light be;') to curly ‘ ’
BBE's "***" marks text its translators left unrendered and is kept.
"""
import argparse
import csv
import io
import json
import re
import sys
import urllib.request
import zipfile
from pathlib import Path

SCROLLMAPPER = "https://raw.githubusercontent.com/scrollmapper/bible_databases/e1b254cef86d0e65b1a5d1a94b8b112d0f296a2c/formats/csv"
EBIBLE_WEB = "https://ebible.org/Scriptures/engwebp_vpl.zip"
PD = "Public Domain"

TRANSLATIONS = {
    "KJV": "King James Version",
    "ASV": "American Standard Version",
    "BBE": "Bible in Basic English",
    "WEB": "World English Bible",
    "YLT": "Young's Literal Translation",
}

# Other languages for side-by-side (bilingual) services: scrollmapper name -> (code, name).
# These are public domain by age; any other scrollmapper name also works with --extra,
# but check its licence before projecting it.
EXTRAS = {
    "FreJND": ("FRD", "Bible J.N. Darby (French, 1885)"),
    "GerElb1905": ("ELB", "Elberfelder Bibel (German, 1905)"),
    "SpaRV1865": ("RV65", "Reina-Valera (Spanish, 1865)"),
}

EXPECTED_VERSES = 30_000  # English sources have 31,086–31,103 non-empty verses; others vary slightly


def clean_kjv(text):
    return text.replace("–", "-")


def clean_asv(text):
    return re.sub(r"\[(?=(Selah|Higgaion))", "", text)


def clean_ylt(text):
    # YLT opens quotes with ` and closes with '; with openers marked, every ' is a closer or apostrophe.
    return text.replace("`", "‘").replace("'", "’")


CLEANERS = {"KJV": clean_kjv, "ASV": clean_asv, "YLT": clean_ylt}


def download(url):
    print(f"↓ {url}")
    # eBible.org answers 403 to Python's default User-Agent.
    req = urllib.request.Request(url, headers={"User-Agent": "HearkenStudio-fetch_bibles/1.0"})
    with urllib.request.urlopen(req, timeout=60) as r:
        return r.read()


def scrollmapper_rows(source):
    text = download(f"{SCROLLMAPPER}/{source}.csv").decode("utf-8-sig")
    for row in csv.DictReader(io.StringIO(text)):
        yield row["Book"], int(row["Chapter"]), int(row["Verse"]), row["Text"]


def web_rows():
    with zipfile.ZipFile(io.BytesIO(download(EBIBLE_WEB))) as z:
        lines = z.read("engwebp_vpl.txt").decode("utf-8-sig").splitlines()
    # Lines look like "GEN 1:1 In the beginning, ..."; books appear in canonical order, so number them 1-66.
    book_ids = {}
    for line in lines:
        m = re.match(r"(\w{3}) (\d+):(\d+) ?(.*)", line)
        if not m:
            continue
        code, chapter, verse, text = m.groups()
        book = book_ids.setdefault(code, len(book_ids) + 1)
        yield book, int(chapter), int(verse), text
    if len(book_ids) != 66:
        sys.exit(f"WEB: expected 66 books, found {len(book_ids)}")


def fetch(code, name, out_dir, force, source=None):
    csv_path = out_dir / f"{code}.csv"
    if csv_path.exists() and not force:
        print(f"✓ {csv_path.name}")
        return
    rows = web_rows() if code == "WEB" else scrollmapper_rows(source or code)
    clean = CLEANERS.get(code, lambda t: t)
    part = csv_path.with_suffix(".csv.part")
    count = 0
    with open(part, "w", newline="", encoding="utf-8") as f:
        w = csv.writer(f)
        w.writerow(["book", "chapter", "verse", "text"])
        for book, chapter, verse, text in rows:
            text = clean(re.sub(r"\s+", " ", text).strip())
            if text:
                w.writerow([book, chapter, verse, text])
                count += 1
    if count < EXPECTED_VERSES:
        part.unlink()
        sys.exit(f"{code}: only {count} verses, expected at least {EXPECTED_VERSES}")
    part.replace(csv_path)
    csv_path.with_suffix(".json").write_text(
        json.dumps({"name": name, "license": PD}, indent=2) + "\n"
    )
    print(f"  {code:6} {count:>7} verses")


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--out", default=str(Path(__file__).resolve().parent.parent / "data" / "translations"))
    ap.add_argument("--force", action="store_true", help="re-download files that already exist")
    ap.add_argument("--extra", action="append", default=[], metavar="NAME",
                    help="also fetch a scrollmapper translation for side-by-side display, e.g. "
                         + ", ".join(EXTRAS) + " (repeatable)")
    args = ap.parse_args()
    out_dir = Path(args.out)
    out_dir.mkdir(parents=True, exist_ok=True)
    for code, name in TRANSLATIONS.items():
        fetch(code, name, out_dir, args.force)
    for source in args.extra:
        code, name = EXTRAS.get(source, (source.upper()[:6], source))
        if source not in EXTRAS:
            print(f"  note: check that {source} is licensed for projection before using it")
        fetch(code, name, out_dir, args.force, source=source)


if __name__ == "__main__":
    main()
