"""velella icon, round 6: low 3/4 view on wet sand; a skewed, flat Liquid Glass sail standing on the crest
of a limp, teardrop float with a translucent skirt and a scallop pattern radiating from the sail's foot."""
import math, random, sys

random.seed(21)
W = 1024
CX, CY = 498, 584
RX, RY = 290, 150
TILT = math.radians(-9)
out = []
a = out.append


def rot(x, y, cx=CX, cy=CY, ang=TILT):
    dx, dy = x - cx, y - cy
    return cx + dx * math.cos(ang) - dy * math.sin(ang), cy + dx * math.sin(ang) + dy * math.cos(ang)


def body_pt(t, scale=1.0, dy=0.0, wav=0.0, seed=0.0):
    c, s = math.cos(t), math.sin(t)
    r = scale * (1 + 0.075 * math.sin(2 * t + 1.2 + seed) + 0.055 * math.sin(3 * t + 0.4) + 0.03 * math.sin(5 * t + 2.2) + wav * math.sin(9 * t + seed) )
    x = CX + RX * r * c * (1 + 0.10 * max(0.0, c) ** 3)          # the right end runs out to a point
    y = CY + RY * r * s * (1 - 0.38 * max(0.0, c) ** 4) + dy
    y += 14 * max(0.0, s) ** 2 * scale                             # the near side sags onto the sand
    return rot(x, y)


def body_path(scale=1.0, dy=0.0, wav=0.0, seed=0.0, n=240):
    pts = [body_pt(2 * math.pi * i / n, scale, dy, wav, seed) for i in range(n)]
    return "M " + " L ".join(f"{x:.1f} {y:.1f}" for x, y in pts) + " Z"


# crest line the sail stands on, and its profile
B0 = rot(CX - 214, CY + 46)
B1 = rot(CX + 196, CY - 78)
H = 196


def sail_top(u):
    hump = 1 + 0.12 * math.sin(2 * math.pi * u + 2.4) + 0.02 * math.sin(13 * u)
    h = H * (math.sin(math.pi * u) ** 0.8) * hump
    x = B0[0] + (B1[0] - B0[0]) * u + 30 * math.sin(math.pi * u) * (0.6 - u)   # leans slightly forward at the left
    y = B0[1] + (B1[1] - B0[1]) * u - h
    return x, y


N = 80
top_pts = [sail_top(i / N) for i in range(N + 1)]
SAIL_TOP = "M " + " L ".join(f"{x:.1f} {y:.1f}" for x, y in top_pts)
SAIL = SAIL_TOP + f" Q {(B0[0]+B1[0])/2:.1f} {(B0[1]+B1[1])/2+14:.1f} {B0[0]:.1f} {B0[1]:.1f} Z"
FOOT = ((B0[0] + B1[0]) / 2 - 20, (B0[1] + B1[1]) / 2 + 4)

a(f'<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="{W}" height="{W}" viewBox="0 0 {W} {W}">')
a(f'''<defs>
  <clipPath id="tile"><rect x="100" y="100" width="824" height="824" rx="185"/></clipPath>
  <clipPath id="sailClip"><path d="{SAIL}"/></clipPath>
  <clipPath id="topClip"><path d="{body_path()}"/></clipPath>
  <linearGradient id="sand" x1="0" y1="0" x2="0" y2="1">
    <stop offset="0" stop-color="#F6F0E6"/>
    <stop offset="0.32" stop-color="#EBDCC6"/>
    <stop offset="0.70" stop-color="#DABFA0"/>
    <stop offset="1" stop-color="#C6A483"/>
  </linearGradient>
  <radialGradient id="wet" cx="0.5" cy="0.5" r="0.5">
    <stop offset="0" stop-color="#6F553B" stop-opacity="0.50"/>
    <stop offset="0.65" stop-color="#8A6E4F" stop-opacity="0.22"/>
    <stop offset="1" stop-color="#8A6E4F" stop-opacity="0"/>
  </radialGradient>
  <linearGradient id="wall" x1="0" y1="0" x2="0" y2="1">
    <stop offset="0" stop-color="#2B3FC6"/>
    <stop offset="0.6" stop-color="#18208A"/>
    <stop offset="1" stop-color="#0B0F48"/>
  </linearGradient>
  <linearGradient id="top" x1="0" y1="0" x2="0" y2="1">
    <stop offset="0" stop-color="#93AAF0"/>
    <stop offset="0.45" stop-color="#5D78E6"/>
    <stop offset="1" stop-color="#2E44C8"/>
  </linearGradient>
  <radialGradient id="pearl" cx="0.5" cy="0.5" r="0.5">
    <stop offset="0" stop-color="#E9EEF6" stop-opacity="0.95"/>
    <stop offset="0.5" stop-color="#B9C8E8" stop-opacity="0.55"/>
    <stop offset="1" stop-color="#8FA6E6" stop-opacity="0"/>
  </radialGradient>
  <linearGradient id="glassTint" x1="0" y1="0" x2="0" y2="1">
    <stop offset="0" stop-color="#FFFFFF" stop-opacity="0.26"/>
    <stop offset="0.6" stop-color="#F2F7FF" stop-opacity="0.10"/>
    <stop offset="1" stop-color="#A9C2FF" stop-opacity="0.28"/>
  </linearGradient>
  <linearGradient id="amber" x1="0" y1="0" x2="1" y2="0">
    <stop offset="0" stop-color="#B98F3A" stop-opacity="0.85"/>
    <stop offset="0.4" stop-color="#D4B15E" stop-opacity="0.60"/>
    <stop offset="1" stop-color="#C29A45" stop-opacity="0.80"/>
  </linearGradient>
  <linearGradient id="haze" x1="0" y1="0" x2="0" y2="1">
    <stop offset="0" stop-color="#FFFFFF" stop-opacity="0.9"/>
    <stop offset="1" stop-color="#FFFFFF" stop-opacity="0"/>
  </linearGradient>
  <linearGradient id="gloss" x1="0" y1="0" x2="0" y2="1">
    <stop offset="0" stop-color="#FFFFFF" stop-opacity="0.24"/>
    <stop offset="0.3" stop-color="#FFFFFF" stop-opacity="0"/>
  </linearGradient>
  <filter id="grain" x="0" y="0" width="100%" height="100%">
    <feTurbulence type="fractalNoise" baseFrequency="1.1" numOctaves="2" seed="4"/>
    <feColorMatrix type="matrix" values="0 0 0 0 0.30  0 0 0 0 0.23  0 0 0 0 0.15  0 0 0 1.1 -0.35"/>
  </filter>
  <filter id="relief" x="0" y="0" width="100%" height="100%">
    <feTurbulence type="fractalNoise" baseFrequency="0.42" numOctaves="3" seed="12" result="h"/>
    <feDiffuseLighting in="h" surfaceScale="1.6" diffuseConstant="1.25" lighting-color="#FFF8EE" result="lit">
      <feDistantLight azimuth="235" elevation="38"/>
    </feDiffuseLighting>
    <feComposite in="lit" in2="SourceGraphic" operator="arithmetic" k1="0.62" k2="0" k3="0.40" k4="0"/>
  </filter>
  <filter id="mottle" x="0" y="0" width="100%" height="100%">
    <feTurbulence type="fractalNoise" baseFrequency="0.018" numOctaves="3" seed="8"/>
    <feColorMatrix type="matrix" values="0 0 0 0 0.45  0 0 0 0 0.34  0 0 0 0 0.22  0 0 0 0.9 -0.32"/>
  </filter>
  <filter id="refract" x="-5%" y="-5%" width="110%" height="110%">
    <feTurbulence type="fractalNoise" baseFrequency="0.006 0.012" numOctaves="1" seed="2" result="w"/>
    <feDisplacementMap in="SourceGraphic" in2="w" scale="12" xChannelSelector="R" yChannelSelector="G" result="d"/>
    <feGaussianBlur in="d" stdDeviation="2.2" result="b"/>
    <feComponentTransfer in="b"><feFuncR type="linear" slope="1.04" intercept="0.05"/><feFuncG type="linear" slope="1.05" intercept="0.06"/><feFuncB type="linear" slope="1.06" intercept="0.08"/></feComponentTransfer>
  </filter>
  <radialGradient id="fadeG" cx="0.5" cy="0.5" r="0.5"><stop offset="0" stop-color="#fff"/><stop offset="0.6" stop-color="#fff" stop-opacity="0.8"/><stop offset="1" stop-color="#fff" stop-opacity="0"/></radialGradient>
  <filter id="b2"><feGaussianBlur stdDeviation="2"/></filter>
  <filter id="b5"><feGaussianBlur stdDeviation="5"/></filter>
  <filter id="b12"><feGaussianBlur stdDeviation="12"/></filter>
  <filter id="b26"><feGaussianBlur stdDeviation="26"/></filter>
</defs>''')

a('<g id="scene" clip-path="url(#tile)">')
a('<rect x="100" y="100" width="824" height="824" fill="url(#sand)"/>')
a('<rect x="100" y="100" width="824" height="824" fill="url(#sand)" filter="url(#relief)"/>')
a('<rect x="100" y="100" width="824" height="824" filter="url(#mottle)" opacity="0.38"/>')
a('<rect x="100" y="100" width="824" height="824" filter="url(#grain)" opacity="0.50"/>')
# scattered grains: dark lithics, white quartz sparkles, a few amber shell bits; denser and larger towards the viewer
for _ in range(2600):
    x, y = random.uniform(100, 924), random.uniform(260, 924)
    depth = (y - 260) / 664
    r = random.uniform(0.6, 1.7) * (0.6 + 0.9 * depth)
    k = random.random()
    col = "#4A3826" if k < 0.45 else ("#FFFDF6" if k < 0.85 else "#C9894A")
    a(f'<circle cx="{x:.1f}" cy="{y:.1f}" r="{r:.2f}" fill="{col}" opacity="{random.uniform(0.25, 0.8) * (0.4 + 0.6*depth):.2f}"/>')
for i in range(6):
    y = 500 + i * 76
    a(f'<path d="M 100 {y} C 320 {y-16} 680 {y+18} 924 {y-4}" stroke="#FFFFFF" stroke-width="12" fill="none" opacity="{0.10 - i*0.012:.3f}" filter="url(#b5)"/>')
a(f'<ellipse cx="{CX-10}" cy="{CY+50}" rx="{RX+110}" ry="{RY+80}" fill="url(#wet)"/>')
a(f'<ellipse cx="{CX+10}" cy="{CY+40}" rx="{RX-10}" ry="{RY-30}" fill="#24170B" opacity="0.38" filter="url(#b12)"/>')
a(f'<ellipse cx="{CX-170}" cy="{CY+128}" rx="140" ry="14" fill="#FFFFFF" opacity="0.32" filter="url(#b5)"/>')

# translucent skirt spreading onto the sand, with a wavy, irregular hem
a(f'<path d="{body_path(scale=1.10, dy=20, wav=0.022, seed=0.7)}" fill="#4E73EC" opacity="0.36" filter="url(#b2)"/>')
a(f'<path d="{body_path(scale=1.10, dy=20, wav=0.022, seed=0.7)}" fill="none" stroke="#2A3CB8" stroke-width="4" opacity="0.35" filter="url(#b2)"/>')
# short, glossy tentacles lying on the sand at the near edge
for (t, ln, w, curl) in [(2.85, 96, 15, -1.0), (2.45, 70, 13, -0.6), (2.05, 84, 14, 0.3), (1.55, 58, 11, 0.8)]:
    x0, y0 = body_pt(t, 0.98, 14)
    dx, dy = math.cos(t) * ln * 0.75, ln * 0.45
    c1 = (x0 + dx * 0.35 + curl * 30, y0 + dy * 0.9)
    c2 = (x0 + dx * 0.85 - curl * 22, y0 + dy * 0.4 + 26)
    e = (x0 + dx, y0 + dy + 10)
    d = f"M {x0:.0f} {y0:.0f} C {c1[0]:.0f} {c1[1]:.0f} {c2[0]:.0f} {c2[1]:.0f} {e[0]:.0f} {e[1]:.0f}"
    a(f'<path d="{d}" stroke="#3B2A18" stroke-width="{w+10}" stroke-linecap="round" fill="none" opacity="0.18" filter="url(#b5)" transform="translate(4 8)"/>')
    a(f'<path d="{d}" stroke="#2F5BE6" stroke-width="{w}" stroke-linecap="round" fill="none" opacity="0.62"/>')
    a(f'<path d="{d}" stroke="#7FA6FF" stroke-width="{w*0.45:.1f}" stroke-linecap="round" fill="none" opacity="0.55"/>')
    a(f'<path d="{d}" stroke="#FFFFFF" stroke-width="{w*0.18:.1f}" stroke-linecap="round" fill="none" opacity="0.65" transform="translate(-2 -3)"/>')
# body: thick near wall, then the crown
a(f'<path d="{body_path(dy=14)}" fill="#0B1048" filter="url(#b2)"/>')
a(f'<path d="{body_path(dy=6)}" fill="#1B2AA6"/>')
a(f'<path d="{body_path()}" fill="url(#top)"/>')
# volume: darker blurred mass towards the near edge, a lifted blurred crown
a('<g clip-path="url(#topClip)">')
a(f'<path d="{body_path(scale=0.92, dy=30)}" fill="#14209A" opacity="0.75" filter="url(#b26)"/>')
a(f'<ellipse cx="{CX-20}" cy="{CY-30}" rx="{RX*0.60:.0f}" ry="{RY*0.50:.0f}" fill="#B7C7F6" opacity="0.50" filter="url(#b26)"/>')
a('</g>')
# scallop: concentric arcs and radial ribs from the sail's foot, on the crown
a('<g clip-path="url(#topClip)">')
a(f'<ellipse cx="{FOOT[0]:.0f}" cy="{FOOT[1]+8:.0f}" rx="{RX*0.72:.0f}" ry="{RY*0.9:.0f}" fill="url(#pearl)"/>')
a(f'<mask id="fade"><ellipse cx="{FOOT[0]:.0f}" cy="{FOOT[1]:.0f}" rx="330" ry="190" fill="url(#fadeG)"/></mask>')
a('<g mask="url(#fade)">')
for i in range(1, 32):
    r = 12 + i * 11.5
    op = 0.18 + 0.40 * (1 - i / 36)
    a(f'<ellipse cx="{FOOT[0]:.1f}" cy="{FOOT[1]:.1f}" rx="{r:.1f}" ry="{r*0.52:.1f}" fill="none" stroke="#132090" stroke-width="2" opacity="{op*0.45:.2f}" transform="rotate(-3 {FOOT[0]:.0f} {FOOT[1]:.0f}) translate(0 1.6)"/>')
    a(f'<ellipse cx="{FOOT[0]:.1f}" cy="{FOOT[1]:.1f}" rx="{r:.1f}" ry="{r*0.52:.1f}" fill="none" stroke="#F1F5FF" stroke-width="1.5" opacity="{op:.2f}" transform="rotate(-3 {FOOT[0]:.0f} {FOOT[1]:.0f})"/>')
a('</g>')
for i in range(56):
    t = 2 * math.pi * i / 56
    x1 = FOOT[0] + 470 * math.cos(t)
    y1 = FOOT[1] + 260 * math.sin(t)
    a(f'<line x1="{FOOT[0]:.1f}" y1="{FOOT[1]:.1f}" x2="{x1:.1f}" y2="{y1:.1f}" stroke="#E4ECFF" stroke-width="1.1" opacity="0.16"/>')
a(f'<path d="{body_path(dy=26)}" fill="#0E1766" opacity="0.50" filter="url(#b12)"/>')
a('</g>')
# lip: dark edge, light frill, wet highlights
a(f'<path d="{body_path()}" fill="none" stroke="#141E86" stroke-width="5" opacity="0.55" filter="url(#b2)"/>')
a(f'<path d="{body_path(scale=0.975)}" fill="none" stroke="#B9CBFF" stroke-width="2" opacity="0.45"/>')
x0, y0 = body_pt(3.6, 0.94); x1, y1 = body_pt(5.3, 0.94); xm, ym = body_pt(4.45, 1.12)
a(f'<path d="M {x0:.0f} {y0:.0f} Q {xm:.0f} {ym:.0f} {x1:.0f} {y1:.0f}" stroke="#FFFFFF" stroke-width="6" fill="none" opacity="0.55" filter="url(#b5)"/>')
x0, y0 = body_pt(2.7, 1.0, 22); x1, y1 = body_pt(0.6, 1.0, 22); xm, ym = body_pt(1.6, 1.12, 22)
a(f'<path d="M {x0:.0f} {y0:.0f} Q {xm:.0f} {ym:.0f} {x1:.0f} {y1:.0f}" stroke="#8FB0FF" stroke-width="3" fill="none" opacity="0.5" filter="url(#b2)"/>')
a('</g>')

a(f'<ellipse cx="{FOOT[0]+10:.0f}" cy="{FOOT[1]+16:.0f}" rx="150" ry="18" fill="#FFFFFF" opacity="0.45" filter="url(#b12)" clip-path="url(#topClip)"/>')
# ---- Liquid Glass sail ----
a('<g clip-path="url(#sailClip)">')
a('<use xlink:href="#scene" filter="url(#refract)"/>')
a(f'<path d="{SAIL}" fill="url(#glassTint)"/>')
# glass thickness: a darker band just inside the far edge and a bright inner bevel along the near/upper edge
a(f'<path d="{SAIL}" fill="none" stroke="#1B2E8F" stroke-width="18" opacity="0.18" filter="url(#b5)" transform="translate(4 6)"/>')
a(f'<path d="{SAIL}" fill="none" stroke="#FFFFFF" stroke-width="14" opacity="0.30" filter="url(#b5)" transform="translate(-3 -3)"/>')
for k in range(1, 10):
    u = k / 10
    bx = B0[0] + (B1[0] - B0[0]) * u
    by = B0[1] + (B1[1] - B0[1]) * u
    tx, ty = sail_top(u)
    a(f'<path d="M {bx:.0f} {by:.0f} Q {bx+(tx-bx)*0.5+6:.0f} {by+(ty-by)*0.5:.0f} {tx:.0f} {ty+8:.0f}" stroke="#FFFFFF" stroke-width="1.4" fill="none" opacity="0.16"/>')
a(f'<path d="M {B0[0]:.0f} {B0[1]:.0f} L {B1[0]:.0f} {B1[1]:.0f}" stroke="#C9DAFF" stroke-width="46" fill="none" opacity="0.30" filter="url(#b12)"/>')
# Liquid Glass inner light: a bright band under the upper edge and a soft lens highlight
hi = [sail_top(u) for u in [i / 40 for i in range(4, 30)]]
a('<path d="M ' + " L ".join(f"{x+4:.0f} {y+22:.0f}" for x, y in hi) + '" stroke="#FFFFFF" stroke-width="20" fill="none" opacity="0.38" filter="url(#b5)"/>')
mx, my = sail_top(0.36)
a(f'<ellipse cx="{mx+10:.0f}" cy="{my+70:.0f}" rx="70" ry="26" fill="#FFFFFF" opacity="0.20" filter="url(#b12)"/>')
a('</g>')
# rim: amber membrane edge, glass edge light inside it, a dark refraction line at the foot, a faint fringe
a(f'<path d="{SAIL_TOP}" stroke="url(#amber)" stroke-width="13" fill="none" stroke-linecap="round" stroke-linejoin="round"/>')
a(f'<path d="{SAIL_TOP}" stroke="#FFFFFF" stroke-width="3.5" fill="none" stroke-linecap="round" opacity="0.95" transform="translate(1 6)"/>')
a(f'<path d="{SAIL_TOP}" stroke="#8DB8FF" stroke-width="2" fill="none" opacity="0.45" transform="translate(-1 10)"/>')
a(f'<path d="M {B0[0]:.0f} {B0[1]:.0f} Q {(B0[0]+B1[0])/2:.0f} {(B0[1]+B1[1])/2+14:.0f} {B1[0]:.0f} {B1[1]:.0f}" stroke="#0E1766" stroke-width="3" fill="none" opacity="0.55"/>')
for (u, v, r) in [(0.30, 0.45, 10), (0.62, 0.30, 7), (0.18, 0.70, 5), (0.78, 0.62, 5)]:
    bx = B0[0] + (B1[0] - B0[0]) * u
    by = B0[1] + (B1[1] - B0[1]) * u
    tx, ty = sail_top(u)
    x, y = bx + (tx - bx) * v, by + (ty - by) * v
    a(f'<circle cx="{x:.0f}" cy="{y:.0f}" r="{r}" fill="none" stroke="#FFFFFF" stroke-width="2" opacity="0.8"/>')
    a(f'<circle cx="{x-r*0.35:.1f}" cy="{y-r*0.35:.1f}" r="{r*0.3:.1f}" fill="#FFFFFF" opacity="0.95"/>')

a('<g clip-path="url(#tile)">')
a('<rect x="100" y="100" width="824" height="320" fill="url(#haze)" opacity="0.7"/>')
a('<rect x="100" y="100" width="824" height="420" fill="url(#gloss)"/>')
a('</g>')
a('<rect x="100" y="100" width="824" height="824" rx="185" fill="none" stroke="#FFFFFF" stroke-opacity="0.5" stroke-width="3"/>')
a('</svg>')
open(sys.argv[1], "w").write("\n".join(out))
