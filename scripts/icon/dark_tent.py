# glowing tentacle fringe drifting in the water (dark appearance), exec'd inside the generator
for i in range(170):
    t = 2 * math.pi * i / 170 + random.uniform(-0.02, 0.02)
    s_ = math.sin(t)
    if s_ < -0.30:
        continue
    x0, y0 = body_pt(t, 0.97, 10)
    ln = random.uniform(40, 96) * (0.45 + 0.65 * max(0, s_))
    dx, dy = math.cos(t) * ln * 0.7, ln * (0.35 + 0.65 * max(0, s_))
    c1x = x0 + dx * 0.4 + random.uniform(-10, 10)
    d = f"M {x0:.1f} {y0:.1f} Q {c1x:.1f} {y0+dy*0.6:.1f} {x0+dx:.1f} {y0+dy:.1f}"
    a(f'<path d="{d}" stroke="#9FD8FF" stroke-width="5" stroke-linecap="round" fill="none" opacity="0.10" filter="url(#b5)"/>')
    a(f'<path d="{d}" stroke="#EAF7FF" stroke-width="{random.uniform(1.4,2.4):.1f}" stroke-linecap="round" fill="none" opacity="{0.35 + 0.4*max(0,s_):.2f}"/>')
    a(f'<circle cx="{x0+dx:.1f}" cy="{y0+dy:.1f}" r="2.2" fill="#FFFFFF" opacity="{0.4 + 0.4*max(0,s_):.2f}"/>')
