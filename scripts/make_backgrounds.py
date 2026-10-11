"""Generate the bundled sample backgrounds in src/assets/backgrounds (original artwork, free to ship).

Usage: python3 scripts/make_backgrounds.py src/assets/backgrounds
"""
import math, random, sys, pathlib
out = pathlib.Path(sys.argv[1])
W, H = 1920, 1080
def svg(body, defs=""):
    return (f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {W} {H}" preserveAspectRatio="xMidYMid slice">'
            f'<defs>{defs}</defs>{body}</svg>\n')

def ridge(rng, base, amp, step, seed_phase):
    pts = [(0, H)]
    x = 0
    y = base
    while x <= W + step:
        y = base + amp * math.sin(x / 260 + seed_phase) + amp * 0.5 * math.sin(x / 97 + seed_phase * 2) + rng.uniform(-amp * .25, amp * .25)
        pts.append((x, y)); x += step
    pts.append((W, H))
    return "M" + " L".join(f"{px:.0f},{py:.0f}" for px, py in pts) + " Z"

files = {}

# Dawn: deep night into a warm horizon glow.
files["dawn"] = svg(
    '<rect width="1920" height="1080" fill="url(#g)"/><ellipse cx="960" cy="1080" rx="900" ry="420" fill="url(#sun)"/>',
    '<linearGradient id="g" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#0f1033"/><stop offset=".45" stop-color="#4c1d95"/>'
    '<stop offset=".78" stop-color="#c2410c"/><stop offset="1" stop-color="#f59e0b"/></linearGradient>'
    '<radialGradient id="sun"><stop offset="0" stop-color="#fde68a" stop-opacity=".95"/><stop offset=".35" stop-color="#fb923c" stop-opacity=".55"/>'
    '<stop offset="1" stop-color="#fb923c" stop-opacity="0"/></radialGradient>')

# Night sky: navy, a soft milky band, scattered stars.
rng = random.Random(7)
stars = "".join(f'<circle cx="{rng.uniform(0,W):.0f}" cy="{rng.uniform(0,H):.0f}" r="{rng.choice([.8,1,1.2,1.6,2.2]):.1f}" '
                f'fill="#fff" opacity="{rng.uniform(.35,1):.2f}"/>' for _ in range(420))
files["night-sky"] = svg(
    f'<rect width="1920" height="1080" fill="url(#g)"/><ellipse cx="900" cy="480" rx="1100" ry="160" fill="#93c5fd" opacity=".10" '
    f'transform="rotate(-18 960 540)" filter="url(#b)"/>{stars}',
    '<radialGradient id="g" cx=".5" cy=".35" r=".9"><stop offset="0" stop-color="#1e3a8a"/><stop offset=".55" stop-color="#0b1440"/>'
    '<stop offset="1" stop-color="#030712"/></radialGradient><filter id="b" x="-50%" y="-50%" width="200%" height="200%"><feGaussianBlur stdDeviation="60"/></filter>')

# Bokeh: out-of-focus warm lights.
rng = random.Random(3)
circles = "".join(f'<circle cx="{rng.uniform(0,W):.0f}" cy="{rng.uniform(0,H):.0f}" r="{rng.uniform(30,150):.0f}" '
                  f'fill="{rng.choice(["#fbbf24","#f97316","#fb7185","#fde68a"])}" opacity="{rng.uniform(.12,.4):.2f}"/>' for _ in range(46))
files["bokeh"] = svg(f'<rect width="1920" height="1080" fill="url(#g)"/><g filter="url(#b)">{circles}</g>',
    '<linearGradient id="g" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="#1c0f0a"/><stop offset="1" stop-color="#2a1208"/></linearGradient>'
    '<filter id="b" x="-20%" y="-20%" width="140%" height="140%"><feGaussianBlur stdDeviation="14"/></filter>')

# Aurora: drifting green/teal/violet bands.
def band(y, amp, phase):
    pts = " ".join(f"{x},{y + amp * math.sin(x / 300 + phase):.0f}" for x in range(-100, W + 101, 40))
    return pts
files["aurora"] = svg(
    '<rect width="1920" height="1080" fill="url(#g)"/><g filter="url(#b)" fill="none" stroke-linecap="round">'
    f'<polyline points="{band(380, 90, 0)}" stroke="#34d399" stroke-width="120" opacity=".55"/>'
    f'<polyline points="{band(470, 70, 1.4)}" stroke="#22d3ee" stroke-width="90" opacity=".45"/>'
    f'<polyline points="{band(300, 60, 2.6)}" stroke="#a78bfa" stroke-width="70" opacity=".35"/></g>',
    '<linearGradient id="g" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#020617"/><stop offset="1" stop-color="#062a2a"/></linearGradient>'
    '<filter id="b" x="-20%" y="-50%" width="140%" height="200%"><feGaussianBlur stdDeviation="45"/></filter>')

# Waves: layered ocean blues.
layers = ""
for i, (c, y) in enumerate([("#1d4ed8", 640), ("#1e40af", 720), ("#1e3a8a", 800), ("#172554", 880), ("#0b1536", 960)]):
    pts = [(x, y + 34 * math.sin(x / 210 + i * 1.3) + 16 * math.sin(x / 80 + i)) for x in range(0, W + 41, 40)]
    d = f"M0,{H} " + " ".join(f"L{x:.0f},{yy:.0f}" for x, yy in pts) + f" L{W},{H} Z"
    layers += f'<path d="{d}" fill="{c}" opacity=".92"/>'
files["waves"] = svg(f'<rect width="1920" height="1080" fill="url(#g)"/>{layers}',
    '<linearGradient id="g" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#020617"/><stop offset=".6" stop-color="#0c4a6e"/></linearGradient>')

# Light rays: beams from above onto a dark stage.
rays = ""
for i in range(13):
    a = math.radians(-60 + i * 10)
    spread = math.radians(2.6 + (i % 3))
    x1, y1 = 960 + 2600 * math.sin(a - spread), 2600 * math.cos(a - spread)
    x2, y2 = 960 + 2600 * math.sin(a + spread), 2600 * math.cos(a + spread)
    rays += f'<polygon points="960,-60 {x1:.0f},{y1:.0f} {x2:.0f},{y2:.0f}" fill="url(#r)" opacity="{.16 + .1 * (i % 2):.2f}"/>'
files["light-rays"] = svg(
    f'<rect width="1920" height="1080" fill="url(#bg)"/><g filter="url(#b)">{rays}</g><ellipse cx="960" cy="0" rx="520" ry="260" fill="url(#glow)"/>',
    '<linearGradient id="r" gradientUnits="userSpaceOnUse" x1="960" y1="0" x2="960" y2="1080"><stop offset="0" stop-color="#fef3c7" stop-opacity=".9"/><stop offset=".7" stop-color="#fef3c7" stop-opacity="0"/></linearGradient>'
    '<linearGradient id="bg" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#1a1408"/><stop offset="1" stop-color="#05060a"/></linearGradient>'
    '<radialGradient id="glow"><stop offset="0" stop-color="#fffbeb" stop-opacity=".7"/><stop offset="1" stop-color="#fffbeb" stop-opacity="0"/></radialGradient>'
    '<filter id="b"><feGaussianBlur stdDeviation="10"/></filter>')

# Mountains: layered ridges at dusk with mist.
rng = random.Random(11)
ridges = ""
for i, (c, base, amp) in enumerate([("#475569", 600, 70), ("#334155", 690, 60), ("#1e293b", 790, 55), ("#0f172a", 900, 45)]):
    ridges += f'<path d="{ridge(rng, base, amp, 30, i * 1.7)}" fill="{c}"/>'
    if i < 3:
        ridges += f'<rect y="{base - 40}" width="1920" height="160" fill="#cbd5e1" opacity=".07" filter="url(#b)"/>'
files["mountains"] = svg(f'<rect width="1920" height="1080" fill="url(#g)"/>{ridges}',
    '<linearGradient id="g" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#1e1b4b"/><stop offset=".55" stop-color="#7c3aed"/>'
    '<stop offset=".8" stop-color="#fda4af"/></linearGradient><filter id="b" x="0" y="-100%" width="100%" height="300%"><feGaussianBlur stdDeviation="30"/></filter>')

# Calvary: a cross on a hill against the sunrise (to the side, leaving room for text).
files["calvary"] = svg(
    '<rect width="1920" height="1080" fill="url(#g)"/><ellipse cx="1500" cy="860" rx="700" ry="360" fill="url(#sun)"/>'
    '<path d="M0,1080 L0,930 C420,880 900,900 1240,850 C1420,825 1560,800 1700,815 C1800,826 1880,850 1920,860 L1920,1080 Z" fill="#120a1e"/>'
    '<g fill="#120a1e"><rect x="1494" y="560" width="22" height="260"/><rect x="1440" y="620" width="130" height="20"/></g>',
    '<linearGradient id="g" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#1e1b4b"/><stop offset=".5" stop-color="#7c2d12"/>'
    '<stop offset="1" stop-color="#f59e0b"/></linearGradient>'
    '<radialGradient id="sun"><stop offset="0" stop-color="#fef3c7" stop-opacity=".9"/><stop offset=".4" stop-color="#fbbf24" stop-opacity=".45"/>'
    '<stop offset="1" stop-color="#fbbf24" stop-opacity="0"/></radialGradient>')

# Linen: light, warm and textured (pair with dark text).
files["linen"] = svg(
    '<rect width="1920" height="1080" fill="url(#g)"/><rect width="1920" height="1080" filter="url(#n)" opacity=".18"/>',
    '<radialGradient id="g" cx=".5" cy=".4" r=".9"><stop offset="0" stop-color="#fdfaf3"/><stop offset="1" stop-color="#e7dcc6"/></radialGradient>'
    '<filter id="n"><feTurbulence type="fractalNoise" baseFrequency=".9" numOctaves="2" seed="4"/><feColorMatrix type="saturate" values="0"/></filter>')

for name, content in files.items():
    (out / f"{name}.svg").write_text(content)
    print(f"{name:12} {len(content)/1024:.1f} KB")
