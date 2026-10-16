"""Generate the bundled emoji catalog and the Twemoji resource folder (slice P1a).

    python3 -I scripts/emoji_catalog.py [--refs DIR]

Reads  <refs>/emoji-toolkit-11.0.0/emoji.json  (JoyPixels emoji-toolkit, JSON under MIT)
and    <refs>/twemoji-16.0.1/assets/svg/*.svg  (jdecked/twemoji, graphics CC-BY 4.0),
writes Sources/VelellaUI/Resources/Emoji/{catalog.json,LICENSE-emoji-toolkit.md} and
       Sources/VelellaUI/Resources/Twemoji/*.svg (+ LICENSE, LICENSE-GRAPHICS).
Deterministic: same inputs, byte-identical outputs. Nothing is fetched.

catalog.json is {"meta": {...}, "categories": [...], "entries": [row, ...]} with
row = [codepoints, names, category index (-1 = none), position, keywords, parent codepoints, tones].
codepoints are lowercase hex joined by "-", fully qualified as in emoji.json; the Swift side derives
the Twemoji file name from them (TwemojiFile). position is the index among the category's base
emoji, or among the parent's tone children. Order of "entries": base emoji by category and
position, then tone children by parent, then the skin-tone modifiers.
"""
import argparse, hashlib, json, os, re, shutil, sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
CATEGORIES = ["people", "nature", "food", "activity", "travel", "objects", "symbols", "flags"]  # Discord's order
MAX_UNICODE = 16  # Discord ships Emoji 16 data
TONE_BASE = 0x1F3FB  # tone 1; tone N = TONE_BASE + N - 1

# --- Hand-written overrides: Discord's names where emoji-toolkit's differ. -----------------------
# Written from names the official client is observed to accept (checked read-only against its
# emoji data for names only; no data copied). Hyphens between letters or digits become underscores
# by rule (blond-haired_woman -> blond_haired_woman), not by row. Rows are keyed by fully qualified
# code points (or by name for DROP_NAMES).
ADD_ALIASES = {
    "1f470-200d-2640-fe0f": ["bride_with_veil"],  # woman_with_veil also answers to bride_with_veil
    "26bd": ["actual_football"],  # soccer ball, for the rest of the world
    "1f4f1": ["iphone"],  # mobile_phone
    "1fa85": ["pinata"],  # piñata without the tilde
    "0030-fe0f-20e3": ["number_0"],  # keycap digits
    "0031-fe0f-20e3": ["number_1"],
    "0032-fe0f-20e3": ["number_2"],
    "0033-fe0f-20e3": ["number_3"],
    "0034-fe0f-20e3": ["number_4"],
    "0035-fe0f-20e3": ["number_5"],
    "0036-fe0f-20e3": ["number_6"],
    "0037-fe0f-20e3": ["number_7"],
    "0038-fe0f-20e3": ["number_8"],
    "0039-fe0f-20e3": ["number_9"],
    "1f51f": ["number_10"],  # keycap_ten
    "00a9-fe0f": ["c"],  # copyright
    "00ae-fe0f": ["r"],  # registered
}
# Discord spells the thumbs-down tone aliases "_1_tone1" (the "-" of "-1" is lost); the plain "-1" stays.
RENAME_NAMES = {f"-1_tone{n}": f"_1_tone{n}" for n in range(1, 6)}
# Aliases Discord does not accept although emoji-toolkit lists them.
DROP_NAMES = {
    "ok_woman",  # person_gesturing_ok; and its tones below
    *(f"ok_woman_tone{n}" for n in range(1, 6)),
    "waving_white_flag", "white_flag", "waving_black_flag", "black_flag",  # flag_white, flag_black
    "chile", "congo", "indonesia", "nigeria", "saudiarabia", "saudi", "turkmenistan", "tuvalu",  # country words
}
# Primary name swaps: [new primary, ...remaining names] replacing the whole list.
SET_NAMES = {"1f1e8-1f1f6": ["flag_sark"]}  # emoji-toolkit calls Sark flag_cq
# Entries Discord does not list although emoji-toolkit does (hair components).
DROP_ENTRIES = {"1f9b0", "1f9b1", "1f9b3", "1f9b2"}
# Position fixes: emoji-toolkit puts handshake after clap, Discord's picker before heart_hands
# (the one 6-position shift in people; every other category order is identical).
MOVE_BEFORE = {"1f91d": "1faf6"}  # handshake before heart_hands
# The five skin-tone modifiers are entries of their own in Discord, named skin-tone-1 .. skin-tone-5.
MODIFIER_NAMES = {f"{TONE_BASE + n:x}": [f"skin-tone-{n + 1}"] for n in range(5)}
# Twemoji file that differs from the naming rule: eye in speech bubble has no FE0F.
FILE_EXCEPTIONS = {"1f441-fe0f-200d-1f5e8-fe0f": "1f441-200d-1f5e8"}


def twemoji_name(cp):
    """Twemoji's file name: lowercase hex, no padding, FE0F dropped unless the sequence has a ZWJ."""
    if cp in FILE_EXCEPTIONS:
        return FILE_EXCEPTIONS[cp]
    parts = [p.lstrip("0") or "0" for p in cp.split("-")]
    if "200d" not in parts:
        parts = [p for p in parts if p != "fe0f"]
    return "-".join(parts)


def names_of(v, cp):
    if cp in SET_NAMES:
        return list(SET_NAMES[cp])
    raw = [v["shortname"]] + v["shortname_alternates"]
    out = []
    for n in raw:
        n = n.strip(":")
        n = re.sub(r"(?<=[a-z0-9])-(?=[a-z0-9])", "_", n)
        n = RENAME_NAMES.get(n, n)
        if n in DROP_NAMES:
            continue
        # two-letter country aliases (":ar:" for flag_ar) are not accepted by Discord
        if v["category"] == "flags" and re.fullmatch(r"[a-z]{2}", n):
            continue
        if n not in out:  # "e-mail" and "e_mail" both map to e_mail
            out.append(n)
    out +=[n for n in ADD_ALIASES.get(cp, []) if n not in out]
    return out


def sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        h.update(f.read())
    return h.hexdigest()


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--refs", default=os.path.join(ROOT, "refs", "third-party"))
    a = ap.parse_args()
    tk = os.path.join(a.refs, "emoji-toolkit-11.0.0")
    tw = os.path.join(a.refs, "twemoji-16.0.1")
    svgdir = os.path.join(tw, "assets", "svg")
    data = json.load(open(os.path.join(tk, "emoji.json"), encoding="utf-8"))
    files = {f[:-4] for f in os.listdir(svgdir) if f.endswith(".svg")}

    kept = {}  # fully qualified cp -> (v, category name or None)
    for key, v in data.items():
        if v["unicode_version"] > MAX_UNICODE or v["category"] == "modifier":
            continue
        cp = v["code_points"]["fully_qualified"]
        if key in DROP_ENTRIES or cp in DROP_ENTRIES:
            continue
        if twemoji_name(cp) not in files:  # digit_*, pound_symbol, asterisk_symbol: no artwork
            continue
        kept[cp] = v

    def kw(v):
        return [k for k in v["keywords"] if not re.fullmatch(r"uc\d+", k)]

    base = {c: [] for c in CATEGORIES}  # category -> [(order, cp)]
    children = []  # (parent cp, [child cps in emoji-toolkit order])
    for cp, v in kept.items():
        if v["code_points"]["diversity_parent"] is not None:
            continue
        cat = "symbols" if v["category"] == "regional" else v["category"]
        # regional indicators sit after the other symbols, in their own order
        base[cat].append((1 if v["category"] == "regional" else 0, v["order"], cp))
    rows = []
    for ci, cat in enumerate(CATEGORIES):
        order = [t[2] for t in sorted(base[cat])]
        for mv, before in MOVE_BEFORE.items():
            if mv in order:
                order.remove(mv)
                order.insert(order.index(before), mv)
        for pos, cp in enumerate(order):
            v = kept[cp]
            rows.append([cp, names_of(v, cp), ci, pos, kw(v), "", []])
            kids = [k["code_points"]["fully_qualified"] for k in
                    (data[c] for c in v["diversity_children"]) if k["code_points"]["fully_qualified"] in kept]
            if kids:
                children.append((cp, kids))
    for parent, kids in sorted(children, key=lambda t: [r[0] for r in rows].index(t[0])):
        # children by tone (lexicographic for two-tone sequences); emoji-toolkit's own order is
        # reversed for some emoji
        def tone_of(cp):
            # the modifiers in the sequence's own order (emoji-toolkit's `diversity` list is sorted);
            # two equal modifiers are one tone, as in the names (kiss_man_man_tone3)
            t = [int(p, 16) - TONE_BASE + 1 for p in cp.split("-") if 0x1F3FB <= int(p, 16) <= 0x1F3FF]
            return t[:1] if len(t) == 2 and t[0] == t[1] else t
        for pos, cp in enumerate(sorted(kids, key=tone_of)):
            v = kept[cp]
            tones = tone_of(cp)
            rows.append([cp, names_of(v, cp), -1, pos, kw(v), parent, tones])
    for h, names in sorted(MODIFIER_NAMES.items()):
        rows.append([h, names, -1, 0, [], "", []])

    # every row must have artwork and every name must be unique
    seen = {}
    for r in rows:
        assert twemoji_name(r[0]) in files, r[0]
        for n in r[1]:
            assert n not in seen, (n, r[0], seen[n])
            seen[n] = r[0]

    out_cat = os.path.join(ROOT, "Sources", "VelellaUI", "Resources", "Emoji")
    out_svg = os.path.join(ROOT, "Sources", "VelellaUI", "Resources", "Twemoji")
    os.makedirs(out_cat, exist_ok=True)
    needed = sorted({twemoji_name(r[0]) for r in rows})
    if os.path.isdir(out_svg):
        shutil.rmtree(out_svg)
    os.makedirs(out_svg)
    for n in needed:
        shutil.copyfile(os.path.join(svgdir, n + ".svg"), os.path.join(out_svg, n + ".svg"))
    for lic in ("LICENSE", "LICENSE-GRAPHICS"):
        shutil.copyfile(os.path.join(tw, lic), os.path.join(out_svg, lic))
    shutil.copyfile(os.path.join(tk, "LICENSE.md"), os.path.join(out_cat, "LICENSE-emoji-toolkit.md"))

    tree = hashlib.sha256()
    for n in needed:
        tree.update(n.encode() + b"\0" + hashlib.sha256(open(os.path.join(out_svg, n + ".svg"), "rb").read()).digest())
    meta = {
        "generator": "scripts/emoji_catalog.py",
        "emoji_toolkit": "11.0.0",
        "emoji_json_sha256": sha256(os.path.join(tk, "emoji.json")),
        "twemoji": "16.0.1",
        "twemoji_svg_count": len(needed),
        "twemoji_svg_tree_sha256": tree.hexdigest(),
        "max_unicode_version": MAX_UNICODE,
    }
    doc = {"meta": meta, "categories": CATEGORIES, "entries": rows}
    with open(os.path.join(out_cat, "catalog.json"), "w", encoding="utf-8", newline="\n") as f:
        f.write(json.dumps(doc, ensure_ascii=False, separators=(",", ":")) + "\n")
    print(f"{len(rows)} entries, {len(needed)} svgs", file=sys.stderr)


if __name__ == "__main__":
    main()
