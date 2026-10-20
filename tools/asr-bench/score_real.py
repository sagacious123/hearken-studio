"""Scores models on a real service recording, against the references that were actually said.

  python score_real.py <refs.txt> results/real-*.json

refs.txt has one reference per line, with roughly when it was said:

  # time     reference
  02:15      John 3:16
  14:40      1 Corinthians 13:4
  1:02:30    Psalm 23:1

Times only need to be within about 20 seconds. Prints, per model: references caught, the ones
missed (with what was heard), and "offered but not said": references the model wrote down that
nobody said, which is what would put a wrong verse in front of the operator.
"""
import json, re, sys
from score import ALIASES, caught, normalise

BOOKS = """genesis exodus leviticus numbers deuteronomy joshua judges ruth 1_samuel 2_samuel 1_kings 2_kings
1_chronicles 2_chronicles ezra nehemiah esther job psalms proverbs ecclesiastes song_of_solomon isaiah
jeremiah lamentations ezekiel daniel hosea joel amos obadiah jonah micah nahum habakkuk zephaniah haggai
zechariah malachi matthew mark luke john acts romans 1_corinthians 2_corinthians galatians ephesians
philippians colossians 1_thessalonians 2_thessalonians 1_timothy 2_timothy titus philemon hebrews james
1_peter 2_peter 1_john 2_john 3_john jude revelation""".split()  # "_" joins a name's words while matching
ONE_CHAPTER = {"obadiah", "philemon", "2 john", "3 john", "jude"}
NEAR = 20     # seconds either side of the given time that still counts as "there"
FILLER = {"chapter", "verse", "verses", "and"}


def seconds(t):
    s = 0
    for part in t.split(":"):
        s = s * 60 + int(part)
    return s


def parse_ref(text):
    m = re.match(r"^\s*(.+?)\s+(\d+)(?::(\d+))?", text)
    if not m:
        raise SystemExit(f"can't read reference: {text!r}")
    book = normalise(m.group(1))
    book = ALIASES.get(book, book)
    ch, v = int(m.group(2)), m.group(3)
    if v is None:  # "Jude 24"
        ch, v = 1, ch
    return {"book": book, "chapter": ch, "verse": int(v)}


def load_refs(path):
    refs = []
    for line in open(path):
        line = line.split("#")[0].strip()
        if line:
            t, rest = line.split(None, 1)
            refs.append((seconds(t), parse_ref(rest)))
    return refs


def found(text):
    """Every reference written in a transcript: (book, chapter, verse)."""
    s = normalise(text)
    for b in sorted((x.replace("_", " ") for x in BOOKS), key=len, reverse=True):
        s = re.sub(rf"\b{b}\b", b.replace(" ", "_"), s)
    toks = s.split()
    out = []
    for i, t in enumerate(toks):
        if t not in BOOKS:
            continue
        nums = [w for w in toks[i + 1:i + 7] if w not in FILLER][:2]
        nums = [int(n) for n in nums if n.isdigit()] if nums and nums[0].isdigit() else []
        book = t.replace("_", " ")
        if len(nums) >= 2 and book not in ONE_CHAPTER:
            out.append((book, nums[0], nums[1]))
        elif nums and book in ONE_CHAPTER:  # "Jude 24", or "Jude 1:24"
            out.append((book, 1, nums[1] if len(nums) == 2 and nums[0] == 1 else nums[0]))
    return out


def app_found(labels):
    """References the app itself found ("Psalm 55:17 (explicit, 93%)", from the replay test):
    (book, chapter, first verse, last verse)."""
    out = []
    for label in labels:
        m = re.match(r"^(.+?) (\d+)(?::(\d+)(?:-(\d+))?)?(?: \((\w+)|$)", label)
        if not m:
            continue
        book = normalise(m.group(1))
        book = ALIASES.get(book, book)
        v1 = int(m.group(3)) if m.group(3) else None
        out.append((book, int(m.group(2)), v1, int(m.group(4)) if m.group(4) else v1, m.group(5) or "said"))
    return out


def app_caught(found, ref):
    return any(b == ref["book"] and c == ref["chapter"] and v1 is not None and v1 <= ref["verse"] <= v2 for b, c, v1, v2, _ in found)


def main(refs_path, paths):
    refs = load_refs(refs_path)
    print(f"{len(refs)} references said\n")
    for p in paths:
        res = json.load(open(p))
        pieces = sorted(((int(k.rsplit("@", 1)[1]), text) for k, text in res["out"].items()), key=lambda x: x[0])
        # From the replay test: score what the app found, not what this script reads in the text.
        app = {int(k.rsplit("@", 1)[1]): app_found(v) for k, v in res.get("refs", {}).items()}
        hit, missed = 0, []
        for t, ref in refs:
            near = [(start, text) for start, text in pieces if start - NEAR <= t <= start + 20 + NEAR]
            if any(app_caught(app[start], ref) if app else caught(text, ref) for start, text in near):
                hit += 1
            else:
                closest = sorted(pieces, key=lambda x: abs(x[0] + 10 - t))[:2]
                heard = " | ".join(text.strip() for _, text in sorted(closest))
                missed.append(f"{t // 60}:{t % 60:02d} {ref['book']} {ref['chapter']}:{ref['verse']}  heard: {heard[:200]}")
        # Written down but never said (near that time). Overlapping pieces repeat a find: count once.
        wrong = {}
        for start, text in pieces:
            finds = [(b, c, v1, k) for b, c, v1, _, k in app[start] if v1 is not None] if app else [f + ("said",) for f in found(text)]
            for book, ch, v, kind in finds:
                said = any(r["book"] == book and r["chapter"] == ch and r["verse"] == v and abs(t - start - 10) <= 60 for t, r in refs)
                key = (book, ch, v, kind)
                if not said and (key not in wrong or start - wrong[key] > 60):
                    wrong[key] = start
        by_kind = {}
        for (_, _, _, kind) in wrong:
            by_kind[kind] = by_kind.get(kind, 0) + 1
        kinds = ", ".join(f"{n} {k}" for k, n in sorted(by_kind.items())) or "none"
        print(f"{res['model']}: {hit}/{len(refs)} caught ({100 * hit / max(len(refs), 1):.0f}%), "
              f"{len(wrong)} offered but not said ({kinds}), {res['audio_seconds'] / res['seconds']:.1f}x real time")
        for m in missed:
            print("   missed  ", m)
        for (book, ch, v, kind), start in sorted(wrong.items(), key=lambda x: x[1]):
            print(f"   not said  {start // 60}:{start % 60:02d} {book} {ch}:{v} ({kind})")
        print()


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2:])
