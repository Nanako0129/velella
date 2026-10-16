"""Builds theme variants of the round-10 generator, renders previews, and exports Icon Composer layers.
usage: python3 -I build_icon.py <outdir>"""
import os, re, subprocess, sys

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = sys.argv[1]
os.makedirs(OUT, exist_ok=True)
src = open(os.path.join(HERE, "gen_a10.py")).read()


def must_replace(s, old, new):
    assert old in s, old[:80]
    return s.replace(old, new, 1)


# group markers (both themes)
src = must_replace(src, "a('<g id=\"scene\" clip-path=\"url(#tile)\">')",
                   "a('<g id=\"scene\" clip-path=\"url(#tile)\">')\na('<g id=\"bg\">')")
src = must_replace(src, "# translucent skirt spreading onto the sand", "a('</g>')\na('<g id=\"body\">')\n# translucent skirt spreading onto the sand")
src = must_replace(src, "stroke=\"#8FB0FF\" stroke-width=\"3\" fill=\"none\" opacity=\"0.5\" filter=\"url(#b2)\"/>')",
                   "stroke=\"#8FB0FF\" stroke-width=\"3\" fill=\"none\" opacity=\"0.5\" filter=\"url(#b2)\"/>')\na('</g>')")
src = must_replace(src, "a(f'<ellipse cx=\"{FOOT[0]+10:.0f}\" cy=\"{FOOT[1]+16:.0f}\" rx=\"150\"",
                   "a('<g id=\"sail\">')\na(f'<ellipse cx=\"{FOOT[0]+10:.0f}\" cy=\"{FOOT[1]+16:.0f}\" rx=\"150\"")
src = must_replace(src, "a('<g clip-path=\"url(#tile)\">')\na('<rect x=\"100\" y=\"100\" width=\"824\" height=\"320\"",
                   "a('</g>')\na('<g id=\"finish\" clip-path=\"url(#tile)\">')\na('<rect x=\"100\" y=\"100\" width=\"824\" height=\"320\"")

light = src
dark = src
extra_defs = ('  <linearGradient id="deep" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#1B5A8F"/>'
              '<stop offset="0.22" stop-color="#0D3366"/><stop offset="0.58" stop-color="#061A3D"/><stop offset="1" stop-color="#020814"/></linearGradient>\n'
              '  <linearGradient id="ray" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#9FD4FF" stop-opacity="0.30"/><stop offset="1" stop-color="#9FD4FF" stop-opacity="0"/></linearGradient>\n'
              '  <radialGradient id="glow" cx="0.5" cy="0.5" r="0.5"><stop offset="0" stop-color="#4C7DFF" stop-opacity="0.55"/><stop offset="1" stop-color="#4C7DFF" stop-opacity="0"/></radialGradient>\n'
              '  <filter id="b2">')
dark = must_replace(dark, '  <filter id="b2">', extra_defs)
# replace the sand block with the deep sea
start = dark.index("a('<rect x=\"100\" y=\"100\" width=\"824\" height=\"824\" fill=\"url(#sand)\"/>')")
end = dark.index("a('</g>')\na('<g id=\"body\">')")
dark = dark[:start] + "exec(open(" + repr(os.path.join(HERE, "dark_bg.py")) + ").read())\n" + dark[end:]
# replace the tentacles lying on sand with a glowing fringe
start = dark.index("# short, glossy tentacles")
end = dark.index("# body: thick near wall")
dark = dark[:start] + "exec(open(" + repr(os.path.join(HERE, "dark_tent.py")) + ").read())\n" + dark[end:]
# no white haze at the top of the deep sea
dark = must_replace(dark, "a('<rect x=\"100\" y=\"100\" width=\"824\" height=\"320\" fill=\"url(#haze)\" opacity=\"0.7\"/>')", "")

for theme, code in (("light", light), ("dark", dark)):
    gen = os.path.join(OUT, f"gen_{theme}.py")
    open(gen, "w").write(code)
    svg = os.path.join(OUT, f"{theme}.svg")
    subprocess.run([sys.executable, "-I", gen, svg], check=True)
    subprocess.run(["rsvg-convert", "-w", "1024", svg, "-o", os.path.join(OUT, f"preview-{theme}.png")], check=True)
    text = open(svg).read()
    # full-bleed layers for Icon Composer: no tile clip, no finish, mapped onto the 1024 canvas
    base = text.replace('<g id="scene" clip-path="url(#tile)">', '<g id="scene">')
    base = re.sub(r'<rect x="100" y="100" width="824" height="824" rx="185" fill="none"[^>]*/>', '', base)
    for layer, show in (("bg", ["bg"]), ("body", ["body"]), ("sail", ["sail"])):
        t = base
        for gid in ("bg", "body", "sail", "finish"):
            if gid not in show:
                t = t.replace(f'<g id="{gid}"', f'<g id="{gid}" display="none"', 1)
        if layer == "sail":
            # the refracting copy of the scene inside the sail must still see bg and body
            t = t.replace('<g id="bg" display="none"', '<g id="bg"', 1).replace('<g id="body" display="none"', '<g id="body"', 1)
            t = t.replace('<g id="scene">', '<g id="scene" display="none">', 1)
        t = t.replace('<defs>', '<defs>', 1)
        body_start = t.index('</defs>') + len('</defs>')
        t = t[:body_start] + '<g transform="scale(1.242718) translate(-100 -100)">' + t[body_start:].replace('</svg>', '</g></svg>')
        p = os.path.join(OUT, f"{layer}-{theme}.svg")
        open(p, "w").write(t)
        subprocess.run(["rsvg-convert", "-w", "1024", p, "-o", os.path.join(OUT, f"{layer}-{theme}.png")], check=True)
print("done", OUT)
