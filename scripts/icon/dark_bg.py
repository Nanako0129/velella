# deep sea background (dark appearance), exec'd inside the generator
a('<rect x="100" y="100" width="824" height="824" fill="url(#deep)"/>')
for (x0, w, op) in [(230, 70, 0.55), (350, 40, 0.40), (460, 90, 0.50), (600, 50, 0.35), (710, 80, 0.45)]:
    a(f'<path d="M {x0} 100 L {x0+w} 100 L {x0+w*2.4+110} 924 L {x0+40} 924 Z" fill="url(#ray)" opacity="{op}" filter="url(#b26)"/>')
for i in range(10):
    y = 116 + i * 13
    a(f'<path d="M 100 {y} C 300 {y-8} 700 {y+8} 924 {y-3}" stroke="#BFE6FF" stroke-width="{3-i*0.2:.1f}" fill="none" opacity="{0.22-i*0.018:.3f}" filter="url(#b2)"/>')
for _ in range(460):
    x, y = random.uniform(100, 924), random.uniform(170, 924)
    r = random.uniform(0.5, 2.6)
    blur = ' filter="url(#b2)"' if r > 1.8 else ''
    a(f'<circle cx="{x:.1f}" cy="{y:.1f}" r="{r:.2f}" fill="#DDF1FF" opacity="{random.uniform(0.12,0.6):.2f}"{blur}/>')
a(f'<ellipse cx="{CX}" cy="{CY+20}" rx="{RX+140}" ry="{RY+120}" fill="url(#glow)"/>')
