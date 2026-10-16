# App icon

`packaging/AppIcon.icon` is the macOS 26 layered icon (Icon Composer format). Its layers, top to bottom:

| Layer | File | Dark appearance |
|---|---|---|
| Sail rim | `rim.svg` (amber edge) | same |
| Sail | `sail.svg`, a white shape at 0.52 opacity marked `glass`; the system draws the Liquid Glass | same |
| Float and tentacles | `body-light.png` | `body-dark.png` (glowing fringe) |
| Background | `bg-light.png` (wet sand) | `bg-dark.png` (deep sea) |

The PNG layers and the previews come from the generator:

```bash
python3 -I scripts/icon/build_icon.py /tmp/velella-icon
```

`icon.json`, `sail.svg` and `rim.svg` were assembled by hand from the generator's sail path. `Sources/Velella/Resources/AppIcon.icns` is the icon as `actool` renders it, used as the Dock icon of the bare executable:

```bash
xcrun actool --compile /tmp/out --platform macosx --minimum-deployment-target 26.0 \
  --app-icon AppIcon --output-partial-info-plist /tmp/out/partial.plist packaging/AppIcon.icon
```
