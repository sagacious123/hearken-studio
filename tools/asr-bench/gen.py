"""Builds the made-up test set: sermon-style lines read by the Mac's voices, each in three rooms.

clips/<id>-<room>.wav (16 kHz mono) and truth.json: what was said and which reference it holds.
Rooms: clean (straight in), hall (echo + crowd hum), band (hall + a soft keyboard pad under it).
"""
import json, os, subprocess, sys

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, "clips")
VOICES = ["Daniel", "Karen", "Moira", "Rishi", "Samantha", "Tessa"]

# (what the preacher says, (book, chapter, verse)). Mostly books Whisper finds hard, said the
# ways preachers say them. None of these is one of the examples in Hearken's own prompt.
LINES = [
    ("Turn with me to Habakkuk chapter two verse four. The just shall live by his faith.", ("Habakkuk", 2, 4)),
    ("Paul writes in Philemon verse six about the sharing of your faith.", ("Philemon", 1, 6)),
    ("Let's read together from Zephaniah three seventeen.", ("Zephaniah", 3, 17)),
    ("Open your Bibles to Lamentations chapter three, verse twenty two.", ("Lamentations", 3, 22)),
    ("Ecclesiastes twelve and thirteen tells us the whole duty of man.", ("Ecclesiastes", 12, 13)),
    ("Look at Philippians chapter two verse five with me.", ("Philippians", 2, 5)),
    ("Colossians three twenty three says whatever you do, work at it with all your heart.", ("Colossians", 3, 23)),
    ("First Thessalonians five seventeen. Pray without ceasing.", ("1 Thessalonians", 5, 17)),
    ("Second Timothy one and seven. God has not given us a spirit of fear.", ("2 Timothy", 1, 7)),
    ("In Deuteronomy chapter eight verse eighteen, remember the Lord your God.", ("Deuteronomy", 8, 18)),
    ("The prophet Obadiah verse fifteen says the day of the Lord is near.", ("Obadiah", 1, 15)),
    ("Malachi three verse ten, bring all the tithes into the storehouse.", ("Malachi", 3, 10)),
    ("Jude verse twenty four, now unto him that is able to keep you from falling.", ("Jude", 1, 24)),
    ("First Peter five and seven, cast all your care upon him.", ("1 Peter", 5, 7)),
    ("Song of Solomon chapter two verse four. His banner over me was love.", ("Song of Solomon", 2, 4)),
    ("Micah six eight. What does the Lord require of you?", ("Micah", 6, 8)),
    ("Hosea chapter four verse six, my people are destroyed for lack of knowledge.", ("Hosea", 4, 6)),
    ("Joel two twenty five. I will restore the years.", ("Joel", 2, 25)),
    ("Psalm one hundred and nineteen verse one hundred and five, thy word is a lamp unto my feet.", ("Psalms", 119, 105)),
    ("Proverbs three five and six, trust in the Lord with all your heart.", ("Proverbs", 3, 5)),
    ("Let's go to Nahum chapter one verse seven. The Lord is good.", ("Nahum", 1, 7)),
    ("Haggai two nine says the glory of this latter house shall be greater.", ("Haggai", 2, 9)),
    ("Second Chronicles seven fourteen. If my people, which are called by my name.", ("2 Chronicles", 7, 14)),
    ("Hebrews eleven verse one, now faith is the substance of things hoped for.", ("Hebrews", 11, 1)),
    ("James chapter one verse five, if any of you lack wisdom, let him ask of God.", ("James", 1, 5)),
    ("Ephesians six eleven, put on the whole armour of God.", ("Ephesians", 6, 11)),
    ("Matthew six thirty three. Seek ye first the kingdom of God.", ("Matthew", 6, 33)),
    ("Jeremiah twenty nine eleven, I know the thoughts that I think toward you.", ("Jeremiah", 29, 11)),
    ("Joshua one verse nine. Be strong and of a good courage.", ("Joshua", 1, 9)),
    ("Isaiah fifty four seventeen, no weapon formed against you shall prosper.", ("Isaiah", 54, 17)),
    ("Third John verse two, I wish above all things that you may prosper.", ("3 John", 1, 2)),
    ("Nehemiah eight and ten, the joy of the Lord is your strength.", ("Nehemiah", 8, 10)),
    ("Galatians six nine. Let us not be weary in well doing.", ("Galatians", 6, 9)),
    ("Acts chapter one verse eight, you shall receive power.", ("Acts", 1, 8)),
    ("Second Corinthians five seventeen, if any man be in Christ he is a new creature.", ("2 Corinthians", 5, 17)),
    ("Zechariah four six, not by might, nor by power, but by my spirit.", ("Zechariah", 4, 6)),
]

# The same speech in three rooms. ffmpeg filters: echo off the walls, a low crowd/air-con hum,
# and for "band" a quiet sustained chord (made here, so no recorded music is involved).
ROOMS = {
    "clean": None,
    "hall": "[0:a]aecho=0.8:0.6:55|110|190:0.35|0.25|0.15,volume=1.0[s];"
            "anoisesrc=color=pink:amplitude=0.035:sample_rate=16000[n];"
            "[s][n]amix=inputs=2:duration=first:normalize=0[o]",
    "band": "[0:a]aecho=0.8:0.6:55|110|190:0.35|0.25|0.15[s];"
            "anoisesrc=color=pink:amplitude=0.035:sample_rate=16000[n];"
            "sine=f=220:sample_rate=16000,volume=0.05[a];sine=f=277.18:sample_rate=16000,volume=0.04[b];"
            "sine=f=329.63:sample_rate=16000,volume=0.04[c];sine=f=110:sample_rate=16000,volume=0.05[d];"
            "[a][b][c][d]amix=inputs=4:normalize=0,tremolo=f=4:d=0.3[m];"
            "[s][n][m]amix=inputs=3:duration=first:normalize=0[o]",
}


def run(cmd):
    subprocess.run(cmd, check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)


def main():
    os.makedirs(OUT, exist_ok=True)
    truth = []
    for i, (text, ref) in enumerate(LINES):
        cid = f"{i:02d}"
        voice = VOICES[i % len(VOICES)]
        dry = os.path.join(OUT, f"{cid}-dry.aiff")
        run(["say", "-v", voice, "-o", dry, text])
        for room, graph in ROOMS.items():
            wav = os.path.join(OUT, f"{cid}-{room}.wav")
            # Half a second of quiet either side, as a live chunk would have.
            pad = "adelay=500,apad=pad_dur=0.5"
            if graph is None:
                run(["ffmpeg", "-y", "-i", dry, "-af", pad, "-ar", "16000", "-ac", "1", wav])
            else:
                run(["ffmpeg", "-y", "-i", dry, "-filter_complex", graph.replace("[0:a]", f"[0:a]{pad},"),
                     "-map", "[o]", "-ar", "16000", "-ac", "1", wav])
        os.remove(dry)
        truth.append({"id": cid, "voice": voice, "text": text, "ref": {"book": ref[0], "chapter": ref[1], "verse": ref[2]}})
        print(cid, voice, ref, file=sys.stderr)
    with open(os.path.join(HERE, "truth.json"), "w") as f:
        json.dump(truth, f, indent=1)
    print(f"{len(truth)} lines x {len(ROOMS)} rooms -> {OUT}")


if __name__ == "__main__":
    main()
