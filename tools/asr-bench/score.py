"""Scores transcripts against truth.json.

  python score.py results/*.json

Each results file is {"model": ..., "seconds": total decode time, "audio_seconds": ...,
"out": {"<id>-<room>": "transcript"}}. Prints, per model and room: references caught,
word error rate, and how many times faster than real time it ran.
"""
import json, os, re, sys
from collections import defaultdict

HERE = os.path.dirname(os.path.abspath(__file__))

ONES = {"zero": 0, "oh": 0, "one": 1, "two": 2, "three": 3, "four": 4, "five": 5, "six": 6, "seven": 7, "eight": 8,
        "nine": 9, "ten": 10, "eleven": 11, "twelve": 12, "thirteen": 13, "fourteen": 14, "fifteen": 15,
        "sixteen": 16, "seventeen": 17, "eighteen": 18, "nineteen": 19}
TENS = {"twenty": 20, "thirty": 30, "forty": 40, "fifty": 50, "sixty": 60, "seventy": 70, "eighty": 80, "ninety": 90}
ORDINAL = {"first": "1", "1st": "1", "second": "2", "2nd": "2", "third": "3", "3rd": "3", "ii": "2", "iii": "3"}  # not "i": that's usually "I"
# Ways a book can come out of a model, all reduced to one spelling.
ALIASES = {"psalm": "psalms", "song of songs": "song of solomon", "philemon": "philemon"}


def words_to_digits(tokens):
    """'one hundred and nineteen' -> '119', 'twenty two' -> '22'. Other words pass through."""
    out, i = [], 0
    while i < len(tokens):
        t = tokens[i]
        if t in ONES or t in TENS or (t == "hundred" and out and out[-1].isdigit()):
            n, used = 0, 0
            j = i
            while j < len(tokens):
                w = tokens[j]
                if w in TENS and n % 100 == 0:
                    n += TENS[w]
                elif w in ONES and n % 10 == 0 and (n % 100 == 0 or n % 100 >= 20):
                    n += ONES[w]
                elif w == "hundred" and 0 < n < 10:
                    n *= 100
                elif w == "and" and n >= 100 and j + 1 < len(tokens) and (tokens[j + 1] in ONES or tokens[j + 1] in TENS):
                    pass
                else:
                    break
                j += 1
            if j == i:
                out.append(t); i += 1
            else:
                out.append(str(n)); i = j
        else:
            out.append(t); i += 1
    return out


def normalise(text):
    t = text.lower().replace("’", "'")
    t = re.sub(r"(\d+):(\d+)", r"\1 \2", t)  # 3:16 -> 3 16
    t = re.sub(r"(\d)-(\d)", r"\1 \2", t)
    t = re.sub(r"[^a-z0-9' ]+", " ", t)
    toks = [ORDINAL.get(w, w) for w in t.split()]
    toks = words_to_digits(toks)
    s = " ".join(toks)
    for a, b in ALIASES.items():
        s = re.sub(rf"\b{a}\b", b, s)
    return s


def caught(text, ref):
    """Is the reference there: the book, then within a few words the chapter, then the verse.
    One-chapter books (Jude, Philemon...) may say only the verse."""
    s = normalise(text)
    book = ref["book"].lower()
    book = ALIASES.get(book, book)
    for m in re.finditer(rf"\b{re.escape(book)}\b", s):
        after = s[m.end():].split()[:8]
        nums = [w for w in after if w.isdigit()]
        ch, v = str(ref["chapter"]), str(ref["verse"])
        if ch in nums and v in nums[nums.index(ch) + 1:]:
            return True
        if ref["chapter"] == 1 and nums and nums[0] == v:
            return True
    return False


FILLER = {"chapter", "verse", "verses", "and"}  # "John chapter 3 verse 16" and "John 3:16" are the same


def wer(ref, hyp):
    r = [w for w in normalise(ref).split() if w not in FILLER]
    h = [w for w in normalise(hyp).split() if w not in FILLER]
    d = list(range(len(h) + 1))
    for i in range(1, len(r) + 1):
        prev, d[0] = d[0], i
        for j in range(1, len(h) + 1):
            cur = min(d[j] + 1, d[j - 1] + 1, prev + (r[i - 1] != h[j - 1]))
            prev, d[j] = d[j], cur
    return d[len(h)], len(r)


def main(paths):
    truth = {t["id"]: t for t in json.load(open(os.path.join(HERE, "truth.json")))}
    rooms = ["clean", "hall", "band"]
    print(f"{'model':34} {'room':6} {'refs caught':>12} {'WER':>6}")
    misses = defaultdict(list)
    for p in paths:
        res = json.load(open(p))
        for room in rooms + ["all"]:
            hit = n = errs = words = 0
            for key, hyp in res["out"].items():
                cid, r = key.split("-")
                if room != "all" and r != room:
                    continue
                t = truth[cid]
                ok = caught(hyp, t["ref"])
                hit += ok; n += 1
                e, w = wer(t["text"], hyp); errs += e; words += w
                if not ok and room == "all":
                    misses[res["model"]].append(f"{key} [{t['voice']}] {t['ref']['book']} {t['ref']['chapter']}:{t['ref']['verse']}  heard: {hyp.strip()}")
            if n:
                print(f"{res['model']:34} {room:6} {hit:>5}/{n:<3} {100*hit/n:3.0f}% {100*errs/max(words,1):5.1f}%")
        print(f"{'':34} speed  {res['audio_seconds']/res['seconds']:.1f}x real time ({res['seconds']:.0f}s for {res['audio_seconds']:.0f}s of audio)")
        print()
    for model, ms in misses.items():
        print(f"--- {model} missed {len(ms)}:")
        for m in ms:
            print("  ", m)


if __name__ == "__main__":
    main(sys.argv[1:])
