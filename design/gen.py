#!/usr/bin/env python3
"""Generate the FigCraft spec for the Labely desktop design."""
import json, re, sys

OUT = sys.argv[1]

# ---------- tokens ----------
C = dict(
    primary="#4C6EF5", primary_d="#3B5BDB", primary_t="#EDF2FF", primary_t2="#DBE4FF",
    bg="#F5F6F8", surface="#FFFFFF", border="#E4E7EC", border_s="#EEF0F3",
    text="#1B2130", text2="#475066", muted="#8A93A6", canvas="#E8EBF0",
    green="#12B886", green_t="#E6FCF5", amber="#F59F00", amber_t="#FFF4E0",
    chrome="#FBFBFC", red="#FA5252",
)
SHADOW_SM = [{"type": "dropShadow", "color": "#1B21301A", "offset": [0, 1], "radius": 3}]
SHADOW_MD = [{"type": "dropShadow", "color": "#1B21301F", "offset": [0, 6], "radius": 18}]
GRID_ON = False
SHADOW_LG = [{"type": "dropShadow", "color": "#1B213040", "offset": [0, 24], "radius": 60}]

# ---------- icons (24 grid, stroke) ----------
ICONS = {
    "to_front": "M5 4h14 M12 20V8 M7 13l5-5 5 5",
    "to_back": "M5 20h14 M12 4v12 M7 11l5 5 5-5",
    "up": "M12 19V5 M6 11l6-6 6 6",
    "down": "M12 5v14 M6 13l6 6 6-6",
    "eye_off": "M3 3l18 18 M10.6 5.2A9.6 9.6 0 0 1 12 5c6 0 10 7 10 7a17 17 0 0 1-3 3.8 M6.5 6.6C3.7 8.5 2 12 2 12s4 7 10 7a9.5 9.5 0 0 0 4-.9",
    "trash": "M4 7h16 M9 7V4h6v3 M6 7l1 13h10l1-13 M10 11v6 M14 11v6",
    "copy": "M8 8h12v12H8z M4 16V4h12",
    "pencil": "M4 20l1-4L16 5l3 3L8 19z M14 7l3 3",
    "globe": "M12 3a9 9 0 1 0 0.01 0 M3 12h18 M12 3c-5 5-5 13 0 18 M12 3c5 5 5 13 0 18",
    "info": "M12 3a9 9 0 1 0 0.01 0 M12 11v6 M12 7.5h.01",
    "download": "M12 4v12 M7 11l5 5 5-5 M4 20h16",
    "battery": "M3 8h16v8H3z M21 11v2",
    "key": "M8 11a4 4 0 1 0 0.01 0 M11 12.5L20 4 M17 7l2 2",
    "sun": "M12 8a4 4 0 1 0 0.01 0 M12 2v2 M12 20v2 M2 12h2 M20 12h2 M5 5l1.5 1.5 M17.5 17.5L19 19 M5 19l1.5-1.5 M17.5 6.5L19 5",
    "pause": "M8 5v14 M16 5v14",
    "scan": "M4 8V4h4 M16 4h4v4 M20 16v4h-4 M8 20H4v-4 M4 12h16",
    "text": "M5 7V5h14v2 M12 5v14 M9 19h6",
    "barcode": "M4 5v14 M7 5v14 M10 5v14 M14 5v14 M16 5v14 M20 5v14",
    "qr": "M4 4h6v6H4z M14 4h6v6h-6z M4 14h6v6H4z M14 14h2v2h-2z M18 18h2v2h-2z M18 14h2 M14 19h1",
    "image": "M4 5h16v14H4z M4 16l5-5 4 4 3-3 4 4 M15 8a1.5 1.5 0 1 0 0.01 0",
    "rect": "M4 6h16v12H4z",
    "ellipse": "M4 12a8 6.5 0 1 0 16 0a8 6.5 0 1 0 -16 0",
    "line": "M5 19L19 5",
    "table": "M4 5h16v14H4z M4 10h16 M4 15h16 M10 5v14",
    "star": "M12 4l2.4 5 5.6.8-4 3.9 1 5.5-5-2.7-5 2.7 1-5.5-4-3.9 5.6-.8z",
    "border": "M3 6h18v12H3z M6 9h12v6H6z",
    "date": "M4 6h16v14H4z M4 10h16 M8 3v5 M16 3v5",
    "serial": "M10 4L8 20 M16 4l-2 16 M5 9h15 M4 15h15",
    "undo": "M9 14L4 9l5-5 M4 9h10a5.5 5.5 0 0 1 0 11h-3",
    "redo": "M15 14l5-5-5-5 M20 9H10a5.5 5.5 0 0 0 0 11h3",
    "al_left": "M4 4v16 M8 7h11v4H8z M8 14h6v4H8z",
    "al_center": "M12 4v16 M6 7h12v4H6z M8 14h8v4H8z",
    "al_right": "M20 4v16 M5 7h11v4H5z M10 14h6v4h-6z",
    "al_top": "M4 4h16 M7 8h4v11H7z M14 8h4v6h-4z",
    "al_middle": "M4 12h16 M7 6h4v12H7z M14 8h4v8h-4z",
    "al_bottom": "M4 20h16 M7 5h4v11H7z M14 10h4v6h-4z",
    "dist_h": "M4 4v16 M20 4v16 M10 8h4v8h-4z",
    "dist_v": "M4 4h16 M4 20h16 M8 10h8v4H8z",
    "group": "M3 7V3h4 M17 3h4v4 M21 17v4h-4 M7 21H3v-4 M8 8h8v8H8z",
    "lock": "M6 11h12v9H6z M8 11V8a4 4 0 0 1 8 0v3",
    "unlock": "M6 11h12v9H6z M8 11V8a4 4 0 0 1 7.5-2",
    "eye": "M2 12s4-7 10-7 10 7 10 7-4 7-10 7S2 12 2 12z M12 9.5a2.5 2.5 0 1 0 0.01 0",
    "layers": "M12 3l9 5-9 5-9-5z M3 13l9 5 9-5",
    "grid": "M4 4h16v16H4z M4 9.3h16 M4 14.6h16 M9.3 4v16 M14.6 4v16",
    "magnet": "M6 4v8a6 6 0 0 0 12 0V4 M6 8h3 M15 8h3",
    "minus": "M6 12h12",
    "plus": "M12 5v14 M5 12h14",
    "printer": "M7 9V3h10v6 M4 9h16v8H4z M7 14h10v7H7z",
    "home": "M3 11l9-7 9 7 M5 9.5V20h14V9.5",
    "templates": "M4 4h7v7H4z M13 4h7v7h-7z M4 13h7v7H4z M13 13h7v7h-7z",
    "folder": "M3 6h6l2 2h10v11H3z",
    "clock": "M12 3a9 9 0 1 0 0.01 0 M12 7v5l3 2",
    "settings": "M4 6h9 M17 6h3 M15 4v4 M4 12h3 M11 12h9 M9 10v4 M4 18h11 M19 18h1 M17 16v4",
    "data": "M4 6c0-1.7 3.6-3 8-3s8 1.3 8 3-3.6 3-8 3-8-1.3-8-3z M4 6v12c0 1.7 3.6 3 8 3s8-1.3 8-3V6 M4 12c0 1.7 3.6 3 8 3s8-1.3 8-3",
    "search": "M11 4a7 7 0 1 0 0.01 0 M16 16l4 4",
    "chev_down": "M6 9l6 6 6-6",
    "chev_left": "M15 6l-6 6 6 6",
    "chev_right": "M9 6l6 6-6 6",
    "close": "M6 6l12 12 M18 6L6 18",
    "file": "M6 3h8l4 4v14H6z M14 3v4h4",
    "open": "M3 7V5h6l2 2h10v12H3z M3 11h18",
    "sparkle": "M12 3l2 6 6 2-6 2-2 6-2-6-6-2 6-2z",
    "link": "M10 14a4 4 0 0 0 6 0l3-3a4 4 0 0 0-6-6l-1 1 M14 10a4 4 0 0 0-6 0l-3 3a4 4 0 0 0 6 6l1-1",
    "check": "M5 12l5 5 9-10",
    "bluetooth": "M7 7l10 10-5 4V3l5 4L7 17",
    "usb": "M12 3v18 M8 7l4-4 4 4 M12 15l-5-3V9 M12 13l5-2V8",
    "upload": "M12 16V4 M7 9l5-5 5 5 M4 20h16",
    "rotate": "M20 12a8 8 0 1 1-2.3-5.7 M20 4v4h-4",
    "flip": "M12 3v18 M9 6L4 18h5z M15 6l5 12h-5z",
    "bold": "M7 5h6a3.5 3.5 0 0 1 0 7H7z M7 12h7a3.5 3.5 0 0 1 0 7H7z",
    "italic": "M10 5h8 M6 19h8 M14 5l-4 14",
    "underline": "M7 4v7a5 5 0 0 0 10 0V4 M5 20h14",
    "t_left": "M4 6h16 M4 10h10 M4 14h16 M4 18h10",
    "t_center": "M4 6h16 M7 10h10 M4 14h16 M7 18h10",
    "t_right": "M4 6h16 M10 10h10 M4 14h16 M10 18h10",
    "more": "M5 12h.01 M12 12h.01 M19 12h.01",
    "bind": "M4 7h10 M4 12h7 M4 17h5 M15 13l3 3 3-3 M18 16V7",
    "warn": "M12 4l9 16H3z M12 10v4 M12 17h.01",
    "agent": "M8 4h8v4H8z M5 8h14v11H5z M9 13h.01 M15 13h.01 M9 16h6",
    "front": "M8 8h12v12H8z M4 4h12v12",
}

NUM = re.compile(r"[A-Za-z]|-?\d*\.?\d+(?:e-?\d+)?")
ARGC = {"M": 2, "L": 2, "H": 1, "V": 1, "C": 6, "S": 4, "Q": 4, "T": 2, "A": 7, "Z": 0}


def scale_path(d, s):
    toks = NUM.findall(d)
    out, cmd, buf = [], None, []
    def fmt(v):
        return ("%.3f" % v).rstrip("0").rstrip(".")
    i = 0
    while i < len(toks):
        t = toks[i]
        if re.match(r"[A-Za-z]", t):
            cmd = t
            out.append(t)
            i += 1
            continue
        n = ARGC[cmd.upper()]
        args = [float(x) for x in toks[i:i + n]]
        i += n
        if cmd.upper() == "A":
            args = [args[0] * s, args[1] * s, args[2], args[3], args[4], args[5] * s, args[6] * s]
            out.append(" ".join(fmt(a) if k not in (2, 3, 4) else str(int(a)) for k, a in enumerate(args)))
        else:
            out.append(" ".join(fmt(a * s) for a in args))
        if cmd == "M":
            cmd = "L"
        elif cmd == "m":
            cmd = "l"
    return " ".join(out)


def icon(name, size=20, color=None, sw=1.6, nm=None):
    s = size / 24
    return {"name": nm or ("ic/" + name), "width": size, "height": size,
            "props": {"fill": "none", "clip": False},
            "children": [{"type": "vector", "name": "glyph", "x": 0, "y": 0,
                          "path": scale_path(ICONS[name], s),
                          "props": {"fill": "none", "stroke": color or C["text2"], "strokeWeight": sw,
                                    "strokeCap": "round", "strokeJoin": "round"}}]}


# ---------- node helpers ----------
def T(text, size=13, color=None, weight="Regular", name=None, **props):
    p = {"fontFamily": "Inter", "fontStyle": weight, "fontSize": size, "fill": color or C["text"]}
    p.update(props)
    return {"type": "text", "name": name or text[:24], "text": text, "props": p}


def F(name, children=None, w=None, h=None, x=None, y=None, **props):
    n = {"name": name, "props": {"fill": "none", **props}}
    if props.get("layout"):
        if h is None and "sizingV" not in props:
            n["props"]["sizingV"] = "hug"
        if w is None and "sizingH" not in props and props.get("layout") == "horizontal":
            n["props"]["sizingH"] = "hug"
    if w is not None: n["width"] = w
    if h is not None: n["height"] = h
    if x is not None: n["x"] = x
    if y is not None: n["y"] = y
    if children: n["children"] = children
    return n


def row(name, children, gap=8, w=None, h=None, **p):
    return F(name, children, w, h, layout="horizontal", gap=gap, crossAlign=p.pop("crossAlign", "center"), **p)


def col(name, children, gap=8, w=None, h=None, **p):
    return F(name, children, w, h, layout="vertical", gap=gap, **p)


def R(name, w, h, fill, x=None, y=None, **p):
    n = {"type": "rectangle", "name": name, "width": w, "height": h, "props": {"fill": fill, **p}}
    if x is not None: n["x"] = x
    if y is not None: n["y"] = y
    return n


def E(name, w, h, fill, x=None, y=None, **p):
    n = R(name, w, h, fill, x, y, **p); n["type"] = "ellipse"; return n


def divider_v(h=20):
    return R("divider", 1, h, C["border"])


def divider_h(name="divider"):
    return R(name, 10, 1, C["border"], sizingH="fill")


def ibtn(ic, active=False, size=32, tip=None, color=None):
    return row("btn/" + (tip or ic), [icon(ic, 18, color or (C["primary"] if active else C["text2"]))],
               gap=0, w=size, h=size, mainAlign="center", cornerRadius=8,
               fill=C["primary_t"] if active else "none")


def button(label, kind="primary", ic=None, h=36, w=None, name=None):
    fg = {"primary": "#FFFFFF", "secondary": C["text"], "ghost": C["primary"]}[kind]
    p = dict(cornerRadius=8, padding=[0, 16, 0, 16 if not ic else 12], mainAlign="center")
    if kind == "primary":
        p.update(fill=C["primary"], effects=[{"type": "dropShadow", "color": "#4C6EF540", "offset": [0, 2], "radius": 6}])
    elif kind == "secondary":
        p.update(fill=C["surface"], stroke=C["border"], strokeWeight=1)
    kids = ([icon(ic, 16, fg, 1.8)] if ic else []) + [T(label, 13, fg, "SemiBold", name="Label")]
    n = row(name or ("Button/" + label), kids, gap=6, h=h, w=w, **p)
    if w is None:
        n["props"]["sizingH"] = "hug"
    return n


def field(label, value, w=None, suffix=None, ic=None, h=32, name=None, focus=False):
    kids = []
    if label:
        kids.append(T(label, 11, C["muted"], "Medium", name="k"))
    kids.append(T(value, 13, C["text"], name="v", sizingH="fill"))
    if suffix:
        kids.append(T(suffix, 11, C["muted"], name="unit"))
    if ic:
        kids.append(icon(ic, 14, C["muted"]))
    n = row(name or ("Field/" + (label or value)), kids, gap=8, h=h, w=w, cornerRadius=7,
            padding=[0, 10, 0, 10], fill=C["surface"],
            stroke=C["primary"] if focus else C["border"], strokeWeight=1.5 if focus else 1)
    if w is None:
        n["props"]["sizingH"] = "fill"
    return n


def section(title, kids, action=None):
    head = [T(title, 11, C["muted"], "SemiBold", name="title", textCase="upper", letterSpacing="4%", sizingH="fill")]
    if action:
        head.append(action)
    return col("Section/" + title, [row("head", head, gap=4, sizingH="fill")] + kids, gap=10,
               padding=[16, 16, 16, 16], strokeSides=[0, 0, 1, 0], stroke=C["border_s"], strokeBottomWeight=1)


def chip(text, active=False, ic=None, name=None, color=None, bg=None):
    kids = ([icon(ic, 14, color or (C["primary"] if active else C["text2"]))] if ic else []) + \
        [T(text, 12, color or (C["primary_d"] if active else C["text2"]), "Medium", name="Label")]
    return row(name or ("Chip/" + text), kids, gap=5, h=28, padding=[0, 10, 0, 10], cornerRadius=14,
               fill=bg or (C["primary_t"] if active else C["surface"]),
               stroke=C["primary_t2"] if active else C["border"], strokeWeight=1)


def segmented(items, active=0, w=None, name="Segmented"):
    kids = []
    for i, it in enumerate(items):
        on = i == active
        inner = [icon(it[1:], 16, C["text"] if on else C["muted"])] if it.startswith("@") else \
            [T(it, 12, C["text"] if on else C["muted"], "SemiBold" if on else "Medium", name="Label")]
        k = row("seg/" + it, inner, gap=0, h=26, mainAlign="center", cornerRadius=6,
                fill=C["surface"] if on else "none", sizingH="fill")
        if on:
            k["props"]["effects"] = SHADOW_SM
        kids.append(k)
    n = row(name, kids, gap=2, h=30, w=w, padding=2, cornerRadius=8, fill="#EEF0F4")
    if w is None:
        n["props"]["sizingH"] = "fill"
    return n


def toggle(on=True):
    return F("Toggle", [E("knob", 14, 14, "#FFFFFF", x=16 if on else 2, y=2, effects=SHADOW_SM)], w=32, h=18,
             fill=C["primary"] if on else "#D0D5DD", cornerRadius=9)


def status_dot(color):
    return E("dot", 8, 8, color)


# ---------- window chrome ----------
def titlebar(tabs, active):
    lights = row("traffic", [E("close", 12, 12, "#FF5F57"), E("min", 12, 12, "#FEBC2E"), E("max", 12, 12, "#28C840")],
                 gap=8, padding=[0, 0, 0, 0])
    tab_nodes = []
    for i, (ic, label) in enumerate(tabs):
        on = i == active
        kids = [icon(ic, 15, C["primary"] if on else C["muted"]), T(label, 12, C["text"] if on else C["text2"],
                                                                    "SemiBold" if on else "Medium", name="Label")]
        if ic != "home":
            kids.append(icon("close", 12, C["muted"]))
        t = row("Tab/" + label, kids, gap=7, h=32, padding=[0, 12, 0, 12], cornerRadius=8,
                fill=C["surface"] if on else "none")
        if on:
            t["props"]["effects"] = SHADOW_SM
        tab_nodes.append(t)
    tab_nodes.append(ibtn("plus", size=28, tip="new-tab", color=C["muted"]))
    right = row("right", [
        row("Printer status", [status_dot(C["green"]), T("B21 Pro", 12, C["text"], "SemiBold"),
                               T("· USB · 82%", 12, C["muted"])],
            gap=6, h=28, padding=[0, 12, 0, 10], cornerRadius=14, fill=C["green_t"]),
        row("Agent status", [icon("agent", 14, C["primary"]), T("MCP", 12, C["primary_d"], "SemiBold")],
            gap=5, h=28, padding=[0, 10, 0, 10], cornerRadius=14, fill=C["primary_t"]),
    ], gap=8)
    return row("Titlebar", [lights, F("sp", w=12, h=1), row("Tabs", tab_nodes, gap=4),
                            F("flex", h=1, sizingH="fill"), right],
               gap=0, h=48, padding=[0, 14, 0, 16], fill="#F0F2F5", sizingH="fill",
               strokeSides=[0, 0, 1, 0], stroke=C["border"], strokeBottomWeight=1)


def statusbar(left, right):
    return row("Statusbar", [T(left, 11, C["muted"], name="left", sizingH="fill"),
                             row("r", [icon("agent", 13, C["muted"]), T(right, 11, C["muted"])], gap=6),
                             divider_v(12), T("HU", 11, C["text2"], "SemiBold")],
               gap=12, h=28, padding=[0, 16, 0, 16], fill=C["chrome"], sizingH="fill",
               strokeSides=[1, 0, 0, 0], stroke=C["border"], strokeTopWeight=1)


# ---------- label artwork (Árcédula) ----------
def barcode(w, h, name="Barcode"):
    bars, x = [], 0
    pattern = [2, 1, 1, 3, 1, 2, 2, 1, 1, 1, 3, 2, 1, 1, 2, 3, 1, 2, 1, 1, 2, 1, 3, 1, 1, 2, 2, 1, 1, 3, 1, 1, 2, 1,
               2, 2, 1, 3, 1, 1, 2, 1, 1, 2, 3, 1, 1, 2, 1, 2]
    unit = w / sum(pattern)
    for i, p in enumerate(pattern):
        if i % 2 == 0:
            bars.append(R("bar", max(1, round(p * unit, 2)), h, "#111111", x=round(x, 2), y=0))
        x += p * unit
    return F(name, bars, w=w, h=h)


def price_label(scale, selected=None, bound=False, name="Label"):
    """50 x 30 mm price tag; scale = px per mm."""
    s = scale
    W, H = 50 * s, 30 * s
    kids = [
        T("Bio alma", round(5.2 * s), "#111111", "Bold", name="Termék", x=3 * s, y=2.6 * s),
        T("Golden · Magyarország", round(2.4 * s), "#333333", name="Leírás", x=3 * s, y=9.2 * s),
        R("Elválasztó", 44 * s, max(1, round(0.3 * s)), "#111111", x=3 * s, y=13 * s),
        T("649", round(8 * s), "#111111", "ExtraBold", name="Ár", x=3 * s, y=14.2 * s, letterSpacing="-2%"),
        T("Ft/kg", round(2.6 * s), "#111111", "SemiBold", name="Egység", x=19.2 * s, y=19.2 * s),
        barcode(18 * s, 7.5 * s, "Vonalkód"),
        T("5 998 001 234 567", round(1.7 * s), "#111111", name="EAN", x=29.6 * s, y=24.4 * s),
        T("2026.11.15.", round(1.9 * s), "#555555", name="Dátum", x=3 * s, y=25.6 * s),
    ]
    kids[5]["x"], kids[5]["y"] = 29 * s, 16.4 * s
    if bound:
        for k in kids:
            if k["name"] in ("Termék", "Ár"):
                pass
    label = F(name, kids, w=W, h=H, fill="#FFFFFF", cornerRadius=2.5 * s, clip=True, effects=SHADOW_MD)
    out = [label]
    if selected:
        sx, sy, sw, sh = [v * s for v in selected]
        out.append(F("Selection", [
            R(f"h{i}", 8, 8, "#FFFFFF", x=hx - 4, y=hy - 4, stroke=C["primary"], strokeWeight=1.5, cornerRadius=2)
            for i, (hx, hy) in enumerate([(0, 0), (sw, 0), (0, sh), (sw, sh), (sw / 2, 0), (sw / 2, sh), (0, sh / 2), (sw, sh / 2)])
        ], w=sw, h=sh, x=sx, y=sy, stroke=C["primary"], strokeWeight=1.5, clip=False))
    return out, W, H


def canvas_area(w, h, sel=True, extra=None, label_scale=10, data_strip=False):
    lw, lh = 50 * label_scale, 30 * label_scale
    area_h = h - (212 if data_strip else 0)
    lx, ly = (w - 20 - lw) / 2 + 20, (area_h - 20 - lh) / 2 + 10
    selbox = sel if isinstance(sel, tuple) else ((2.4, 13.6, 24.5, 9.6) if sel else None)
    art, _, _ = price_label(label_scale, selected=selbox)
    for a in art:
        a["x"] = lx + a.get("x", 0)
        a["y"] = ly + a.get("y", 0)
    # printable area
    pa = R("Nyomtatható terület", lw - 2 * 1.2 * label_scale, lh - 2 * 1.2 * label_scale, "none",
           x=lx + 1.2 * label_scale, y=ly + 1.2 * label_scale, stroke="#4C6EF566", strokeWeight=1, dashes=[4, 4])
    # rulers
    ticks_top = [R("t", 1, 6 if (i % 5) else 10, "#B8BFCC", x=lx + i * label_scale, y=20 - (6 if (i % 5) else 10))
                 for i in range(0, 51)]
    nums_top = [T(str(i), 9, C["muted"], x=lx + i * label_scale + 3, y=2, name="n") for i in range(0, 51, 10)]
    ticks_left = [R("t", 6 if (i % 5) else 10, 1, "#B8BFCC", x=20 - (6 if (i % 5) else 10), y=ly + i * label_scale)
                  for i in range(0, 31)]
    nums_left = [T(str(i), 9, C["muted"], x=3, y=ly + i * label_scale + 2, name="n") for i in range(10, 31, 10)]
    ruler_top = F("Vonalzó/felső", ticks_top + nums_top +
                  ([R("hl", selbox[2] * label_scale, 20, "#4C6EF51A", x=lx + selbox[0] * label_scale, y=0)] if selbox else []),
                  w=w, h=20, x=0, y=0, fill=C["chrome"], stroke=C["border"], strokeSides=[0, 0, 1, 0],
                  strokeBottomWeight=1, clip=True)
    ruler_left = F("Vonalzó/bal", ticks_left + nums_left + ([R("hl", 20, selbox[3] * label_scale, "#4C6EF51A", x=0, y=ly + selbox[1] * label_scale)] if selbox else []), w=20, h=area_h, x=0, y=0, fill=C["chrome"],
                   stroke=C["border"], strokeSides=[0, 1, 0, 0], strokeRightWeight=1, clip=True)
    corner = R("corner", 20, 20, C["chrome"], x=0, y=0)
    # floating label chip
    chip_lbl = row("Címkeinfó", [icon("rect", 14, C["text2"]), T("50 × 30 mm", 12, C["text"], "SemiBold"),
                                 divider_v(14), E("paper", 10, 10, "#FFFFFF", stroke=C["border"], strokeWeight=1),
                                 T("Fehér · Résközös", 12, C["text2"]), icon("chev_down", 14, C["muted"])],
                   gap=8, h=34, padding=[0, 12, 0, 12], cornerRadius=17, fill=C["surface"], effects=SHADOW_SM,
                   x=(w - 300) / 2 + 10, y=ly + lh + 28)
    zoom = row("Nagyítás", [ibtn("minus", size=28), T("200%", 12, C["text"], "SemiBold", textAlign="center"),
                            ibtn("plus", size=28), divider_v(14), ibtn("grid", size=28, tip="fit")],
               gap=4, h=36, padding=[0, 4, 0, 4], cornerRadius=10, fill=C["surface"], effects=SHADOW_SM,
               x=w - 214, y=area_h - 52)
    kids = [pa] + art + [ruler_top, ruler_left, corner, chip_lbl, zoom] + (extra(lx, ly, label_scale) if callable(extra) else (extra or []))
    return F("Vászon", kids, w=w, h=h, fill=C["canvas"], clip=True, sizingH="fill")


# ---------- editor toolbar ----------
def toolbar():
    g = lambda name, kids: row(name, kids, gap=2)
    return row("Eszköztár", [
        g("Előzmények", [ibtn("undo"), ibtn("redo")]), divider_v(),
        g("Igazítás", [ibtn(i) for i in ("al_left", "al_center", "al_right", "al_top", "al_middle", "al_bottom")]),
        divider_v(),
        g("Elosztás", [ibtn("dist_h"), ibtn("dist_v")]), divider_v(),
        g("Elrendezés", [ibtn("group"), ibtn("front"), ibtn("rotate"), ibtn("flip"), ibtn("lock")]),
        F("flex", h=1, sizingH="fill"),
        g("Nézet", [ibtn("grid", active=GRID_ON), ibtn("magnet", active=True)]), divider_v(),
        button("Nyomtatási kép", "secondary", ic="eye", h=32),
        button("Nyomtatás", "primary", ic="printer", h=32),
    ], gap=10, h=52, padding=[0, 16, 0, 12], fill=C["surface"], sizingH="fill",
        strokeSides=[0, 0, 1, 0], stroke=C["border"], strokeBottomWeight=1)


def rail(active):
    items = [("plus", "Beszúrás"), ("layers", "Rétegek"), ("data", "Adatok"), ("templates", "Sablonok")]
    kids = []
    for ic, lab in items:
        on = lab == active
        kids.append(col("Rail/" + lab, [row("i", [icon(ic, 20, C["primary"] if on else C["text2"])], gap=0, w=40, h=32,
                                            mainAlign="center", cornerRadius=8, fill=C["primary_t"] if on else "none"),
                                        T(lab, 10, C["primary_d"] if on else C["muted"], "SemiBold" if on else "Medium",
                                          name="Label", textAlign="center")],
                        gap=4, w=64, crossAlign="center"))
    kids.append(F("flex", w=1, sizingV="fill"))
    kids.append(col("Rail/Beállítások", [row("i", [icon("settings", 20)], gap=0, w=40, h=32, mainAlign="center")],
                    gap=0, w=64, crossAlign="center"))
    return col("Oldalsáv", kids, gap=14, w=72, padding=[14, 4, 14, 4], fill=C["chrome"], crossAlign="center",
               sizingV="fill", strokeSides=[0, 1, 0, 0], stroke=C["border"], strokeRightWeight=1)


def insert_panel():
    tiles = [("text", "Szöveg"), ("barcode", "Vonalkód"), ("qr", "QR-kód"), ("image", "Kép"),
             ("table", "Táblázat"), ("star", "Ikon"), ("rect", "Téglalap"), ("ellipse", "Ellipszis"),
             ("line", "Vonal"), ("border", "Keret"), ("date", "Dátum"), ("serial", "Sorszám")]
    grid_rows = []
    for r_ in range(4):
        grid_rows.append(row("row", [
            col("Tile/" + lab, [row("ic", [icon(ic, 22, C["text"])], gap=0, mainAlign="center", sizingH="fill"),
                                T(lab, 11, C["text2"], "Medium", name="Label", textAlign="center")],
                gap=6, h=68, crossAlign="center", mainAlign="center", cornerRadius=10, fill=C["surface"],
                stroke=C["border"], strokeWeight=1, sizingH="fill")
            for ic, lab in tiles[r_ * 3:r_ * 3 + 3]], gap=8, sizingH="fill"))
    layers = [("text", "Termék", False, False), ("text", "Leírás", False, False),
              ("text", "Ár", True, False), ("text", "Ft/kg", False, False),
              ("barcode", "Vonalkód", False, False), ("date", "Dátum", False, True)]
    layer_rows = []
    for ic, lab, on, locked in layers:
        layer_rows.append(row("Layer/" + lab, [
            icon(ic, 15, C["primary"] if on else C["muted"]),
            T(lab, 12, C["primary_d"] if on else C["text"], "SemiBold" if on else "Regular", name="Label", sizingH="fill"),
            icon("lock" if locked else "eye", 14, C["muted"] if locked or on else "#C3C9D4"),
        ], gap=8, h=30, padding=[0, 8, 0, 8], cornerRadius=6, fill=C["primary_t"] if on else "none", sizingH="fill"))
    return col("Bal panel", [
        row("Panelfej", [T("Beszúrás", 15, C["text"], "Bold", sizingH="fill"), ibtn("search", size=28)],
            gap=6, h=52, padding=[0, 12, 0, 16], sizingH="fill"),
        col("Elemek", grid_rows + [
            row("Kód képből", [icon("qr", 16, C["primary"]), T("Kód beolvasása képből…", 12, C["primary_d"], "SemiBold")],
                gap=8, h=34, mainAlign="center", cornerRadius=8, fill=C["primary_t"], sizingH="fill")],
            gap=8, padding=[0, 16, 16, 16], sizingH="fill"),
        divider_h(),
        col("Rétegek", [row("h", [T("RÉTEGEK", 11, C["muted"], "SemiBold", letterSpacing="4%", sizingH="fill"),
                                  T("6", 11, C["muted"], "SemiBold")], gap=4, sizingH="fill")] + layer_rows,
            gap=2, padding=[14, 8, 8, 8], sizingH="fill"),
    ], gap=0, w=268, fill=C["surface"], sizingV="fill", strokeSides=[0, 1, 0, 0], stroke=C["border"],
        strokeRightWeight=1)


def arrange_section():
    def b(ic, lab):
        return col("z/" + lab, [row("i", [icon(ic, 16, C["text2"])], gap=0, mainAlign="center", sizingH="fill"),
                               T(lab, 10, C["text2"], "Medium", textAlign="center")],
                   gap=4, padding=[8, 0, 8, 0], cornerRadius=8, stroke=C["border"], strokeWeight=1, sizingH="fill")
    return section("Elrendezés", [
        row("z", [b("to_front", "Előtérbe"), b("up", "Előrébb"), b("down", "Hátrébb"), b("to_back", "Háttérbe")], gap=6,
            sizingH="fill"),
        row("lock", [T("Zárolva", 12, C["text2"], sizingH="fill"), toggle(False)], gap=8, sizingH="fill"),
        row("hide", [T("Rejtett", 12, C["text2"], sizingH="fill"), toggle(False)], gap=8, sizingH="fill"),
    ])


def props_panel():
    return col("Jobb panel", [
        row("Fül", [segmented(["Elem", "Címke"], 0)], gap=0, padding=[12, 16, 4, 16], sizingH="fill"),
        row("Kijelölés", [row("kind", [icon("text", 16, C["primary"])], gap=0, w=32, h=32, mainAlign="center",
                                 cornerRadius=8, fill=C["primary_t"]),
                          col("t", [T("Ár", 14, C["text"], "Bold"), T("Szöveg · kötve: Ár oszlop", 11, C["muted"])], gap=2,
                              sizingH="fill"),
                          ibtn("more", size=28)], gap=10, padding=[10, 16, 10, 16], sizingH="fill"),
        section("Helyzet és méret", [
            row("r1", [field("X", "2,4", suffix="mm"), field("Y", "13,6", suffix="mm")], gap=8, sizingH="fill"),
            row("r2", [field("SZ", "24,5", suffix="mm"), field("M", "9,6", suffix="mm")], gap=8, sizingH="fill"),
            row("r3", [field(None, "0°", ic="rotate"), row("flip", [ibtn("flip", size=32), ibtn("lock", size=32)],
                                                           gap=4)], gap=8, sizingH="fill"),
        ]),
        section("Szöveg", [
            field(None, "Inter", ic="chev_down"),
            row("r", [field(None, "ExtraBold", ic="chev_down"), field(None, "32", suffix="pt", w=86)], gap=8,
                sizingH="fill"),
            row("r2", [segmented(["@bold", "@italic", "@underline"], 0, name="Stílus"),
                       segmented(["@t_left", "@t_center", "@t_right"], 0, name="Igazítás")], gap=8, sizingH="fill"),
            row("auto", [T("Méret a dobozhoz igazítva", 12, C["text2"], sizingH="fill"), toggle(True)], gap=8,
                sizingH="fill"),
        ]),
        section("Tartalom", [
            row("Kötés", [icon("link", 15, C["primary"]), T("Ár", 12, C["primary_d"], "SemiBold", sizingH="fill"),
                          T("CSV oszlop", 11, C["muted"]), icon("close", 12, C["muted"])],
                gap=8, h=34, padding=[0, 10, 0, 10], cornerRadius=8, fill=C["primary_t"], sizingH="fill"),
            row("ins", [chip("Dátum", ic="date"), chip("Időpont", ic="clock"), chip("Sorszám", ic="serial")], gap=4),
        ]),
        arrange_section(),
    ], gap=0, w=300, fill=C["surface"], sizingV="fill", strokeSides=[0, 0, 0, 1], stroke=C["border"],
        strokeLeftWeight=1)


def data_panel():
    cols = [("Termék", "Bio alma", True), ("Leírás", "Golden · Magyarország", True), ("Ár", "649", True),
            ("Egység", "Ft/kg", False), ("EAN", "5998001234567", True), ("Kategória", "Gyümölcs", False)]
    rows_ = [row("Col/" + n, [
        icon("bind" if b else "data", 15, C["primary"] if b else C["muted"]),
        col("t", [T(n, 12, C["text"], "SemiBold"), T(v, 11, C["muted"])], gap=1, sizingH="fill"),
        (row("b", [icon("check", 12, C["green"]), T("Kötve", 11, C["green"], "SemiBold")], gap=3, h=22,
             padding=[0, 8, 0, 6], cornerRadius=11, fill=C["green_t"]) if b else
         row("b", [T("Kötés", 11, C["primary_d"], "SemiBold")], gap=0, h=22, padding=[0, 10, 0, 10], cornerRadius=11,
             stroke=C["primary_t2"], strokeWeight=1)),
    ], gap=10, h=44, padding=[0, 10, 0, 10], cornerRadius=8, fill="#FAFBFC" if b else "none", sizingH="fill")
        for n, v, b in cols]
    return col("Bal panel", [
        row("Panelfej", [T("Adatok", 15, C["text"], "Bold", sizingH="fill"), ibtn("more", size=28)],
            gap=6, h=52, padding=[0, 12, 0, 16], sizingH="fill"),
        col("Forrás", [
            row("Fájl", [row("i", [icon("file", 18, C["green"])], gap=0, w=36, h=36, mainAlign="center",
                                cornerRadius=8, fill=C["green_t"]),
                         col("t", [T("termekek.csv", 13, C["text"], "SemiBold"),
                                   T("24 rekord · 6 oszlop · UTF-8", 11, C["muted"])], gap=2, sizingH="fill"),
                         ibtn("close", size=28, color=C["muted"])],
                gap=10, padding=10, cornerRadius=10, stroke=C["border"], strokeWeight=1, sizingH="fill"),
            T("Húzz egy oszlopot a címke egyik elemére, vagy válaszd a Kötés gombot.", 11, C["muted"],
              lineHeight="150%", sizingH="fill"),
        ], gap=10, padding=[0, 16, 12, 16], sizingH="fill"),
        divider_h(),
        col("Oszlopok", [T("OSZLOPOK", 11, C["muted"], "SemiBold", letterSpacing="4%")] + rows_,
            gap=4, padding=[14, 8, 8, 8], sizingH="fill"),
        divider_h(),
        col("Sorszám", [
            row("h", [T("SORSZÁMOK", 11, C["muted"], "SemiBold", letterSpacing="4%", sizingH="fill"), toggle(False)],
                gap=4, sizingH="fill"),
            row("r", [field("Kezdő", "1"), field("Lépés", "1"), field("Jegy", "4")], gap=6, sizingH="fill"),
        ], gap=10, padding=[14, 16, 14, 16], sizingH="fill"),
    ], gap=0, w=268, fill=C["surface"], sizingV="fill", strokeSides=[0, 1, 0, 0], stroke=C["border"],
        strokeRightWeight=1)


def data_strip(w):
    hdr = ["#", "Termék", "Leírás", "Ár", "Egység", "EAN"]
    widths = [44, 150, 230, 80, 80, 170]
    data = [("1", "Bio alma", "Golden · Magyarország", "649", "Ft/kg", "5998001234567"),
            ("2", "Körte", "Vilmos · Magyarország", "899", "Ft/kg", "5998001234574"),
            ("3", "Szilva", "Besztercei · Szatmár", "749", "Ft/kg", "5998001234581"),
            ("4", "Szőlő", "Irsai Olivér · Badacsony", "1290", "Ft/kg", "5998001234598")]

    def tr(vals, head=False, on=False, idx=0):
        return row("Sor/" + vals[0], [
            T(v, 11 if head else 12, C["muted"] if head else (C["primary_d"] if on else C["text"]),
              "SemiBold" if head or on else "Regular", name="c", width=widths[i], textAutoResize="height")
            for i, v in enumerate(vals)], gap=0, h=28 if head else 30, padding=[0, 12, 0, 12],
            fill=C["primary_t"] if on else ("#F7F8FA" if head else "none"), sizingH="fill",
            strokeSides=[0, 0, 1, 0], stroke=C["border_s"], strokeBottomWeight=1)

    return col("Rekordok", [
        row("fej", [T("Rekordok", 13, C["text"], "Bold"), chip("24 rekord"), F("flex", h=1, sizingH="fill"),
                    row("nav", [ibtn("chev_left", size=28), T("1 / 24", 12, C["text"], "SemiBold"),
                                ibtn("chev_right", size=28)], gap=4),
                    field(None, "Rekordok: 1-24", w=170, ic="chev_down", h=28)],
            gap=10, h=44, padding=[0, 12, 0, 16], sizingH="fill"),
        tr(hdr, head=True)] + [tr(d, on=(i == 0)) for i, d in enumerate(data)],
        gap=0, w=w - 32, h=196, x=16, y=0, fill=C["surface"], cornerRadius=12, clip=True, effects=SHADOW_SM)


def editor(name, left, tab="Árcédula", with_data=False, overlay=None, right=None, rail_active=None, sel=True,
           extra=None, status=None):
    W, H = 1440, 900
    body_h = H - 48 - 52 - 28
    cw = W - 72 - 268 - 300
    extra = extra or []
    if with_data:
        strip = data_strip(cw)
        strip["y"] = body_h - 196 - 16
        extra.append(strip)
    canvas = canvas_area(cw, body_h, sel=sel, data_strip=with_data, extra=extra)
    body = row("Törzs", [rail(rail_active or ("Adatok" if with_data else "Beszúrás")), left, canvas, right or props_panel()],
               gap=0, h=body_h, crossAlign="min", sizingH="fill")
    kids = [titlebar([("home", "Kezdőlap"), ("file", tab + ".labely"), ("file", "Kábelzászló.labely")], 1),
            toolbar(), body, statusbar(status or "6 elem · 1 kijelölve · Nincs hiba · Mentve 2 perce",
                                       "Ügynökök csatlakozhatnak: 127.0.0.1:7878/mcp")]
    fr = col(name, kids, gap=0, w=W, h=H, fill=C["bg"], clip=True, cornerRadius=12)
    if overlay:
        fr["children"].append(overlay)
    return fr


# ---------- print dialog ----------
def print_dialog():
    W, H = 1440, 900
    dw, dh = 860, 600
    art, lw, lh = price_label(7)
    prev = F("preview-art", art, w=lw, h=lh)
    left = col("Előnézet", [
        row("h", [T("Nyomtatási kép", 13, C["text"], "Bold", sizingH="fill"), chip("1 bites", ic="eye")], gap=6,
            sizingH="fill"),
        col("stage", [prev, T("Így kerül a papírra: 203 dpi, fekete-fehér", 11, C["muted"], sizingH="hug", textAlign="center")],
            gap=14, mainAlign="center", crossAlign="center", fill=C["canvas"], cornerRadius=12, sizingH="fill",
            sizingV="fill"),
        row("nav", [ibtn("chev_left", size=30), T("1. rekord / 24", 12, C["text"], "SemiBold", sizingH="fill",
                                                   textAlign="center"), ibtn("chev_right", size=30)],
            gap=6, sizingH="fill"),
    ], gap=12, w=440, padding=20, fill="#FAFBFC", sizingV="fill", strokeSides=[0, 1, 0, 0], stroke=C["border"],
        strokeRightWeight=1)

    def lab(t):
        return T(t, 12, C["text2"], "SemiBold")

    printer_sel = row("Nyomtató", [
        row("i", [icon("printer", 18, C["primary"])], gap=0, w=36, h=36, mainAlign="center", cornerRadius=8,
            fill=C["primary_t"]),
        col("t", [T("NIIMBOT B21 Pro", 13, C["text"], "SemiBold"),
                  row("s", [status_dot(C["green"]), T("Csatlakozva USB-n · 82% · 50×30 papír", 11, C["muted"])], gap=6)],
            gap=3, sizingH="fill"),
        icon("chev_down", 16, C["muted"])], gap=10, padding=10, cornerRadius=10, stroke=C["primary"],
        strokeWeight=1.5, sizingH="fill")
    stepper = row("Példányszám", [ibtn("minus", size=30), T("1", 14, C["text"], "SemiBold", textAlign="center",
                                                           width=40, textAutoResize="height"),
                                  ibtn("plus", size=30)], gap=0, h=36, w=124, padding=3, cornerRadius=8, mainAlign="spaceBetween",
                  stroke=C["border"], strokeWeight=1)
    density = col("Sötétség", [
        row("h", [lab("Sötétség"), F("f", h=1, sizingH="fill"), T("3 / 5", 12, C["text"], "SemiBold")], gap=4,
            sizingH="fill"),
        F("slider", [R("track", 380, 4, "#E4E7EC", x=0, y=8, cornerRadius=2),
                     R("fill", 190, 4, C["primary"], x=0, y=8, cornerRadius=2),
                     E("knob", 20, 20, "#FFFFFF", x=180, y=0, stroke=C["primary"], strokeWeight=2, effects=SHADOW_SM)],
          w=380, h=20, sizingH="fill"),
    ], gap=8, sizingH="fill")
    right = col("Beállítások", [
        col("g1", [lab("Nyomtató"), printer_sel], gap=8, sizingH="fill"),
        row("g2", [col("c", [lab("Példányszám"), stepper], gap=8, w=124),
                   col("c2", [lab("Papírtípus"), field(None, "Résközös", ic="chev_down", h=36)], gap=8,
                       sizingH="fill")], gap=16, sizingH="fill"),
        density,
        col("g4", [lab("Rekordok"), field(None, "1-24", h=36, focus=True),
                   T("Például 1,3,5-7. Üresen az összes rekordot nyomtatja.", 11, C["muted"])], gap=8,
            sizingH="fill"),
        row("sum", [icon("check", 16, C["green"]), T("Ellenőrzés: nincs hiba", 12, C["text2"], "Medium",
                                                     sizingH="fill"),
                    T("24 címke", 13, C["text"], "Bold")], gap=8, h=40, padding=[0, 12, 0, 12], cornerRadius=8,
            fill=C["green_t"], sizingH="fill"),
    ], gap=20, padding=24, sizingH="fill", sizingV="fill")
    head = row("Fej", [T("Nyomtatás", 17, C["text"], "Bold", sizingH="fill"), ibtn("close", size=30)],
               gap=8, h=60, padding=[0, 16, 0, 24], sizingH="fill", strokeSides=[0, 0, 1, 0], stroke=C["border"],
               strokeBottomWeight=1)
    foot = row("Lábléc", [row("hint", [icon("sparkle", 14, C["muted"]),
                                       T("Ügynökök csak engedéllyel nyomtathatnak", 11, C["muted"])], gap=6),
                          F("f", h=1, sizingH="fill"), button("Mégse", "secondary"),
                          button("24 címke nyomtatása", "primary", ic="printer")],
               gap=10, h=68, padding=[0, 24, 0, 24], sizingH="fill", fill="#FAFBFC", strokeSides=[1, 0, 0, 0],
               stroke=C["border"], strokeTopWeight=1)
    dialog = col("Nyomtatás ablak", [head, row("tartalom", [left, right], gap=0, crossAlign="min",
                                                  sizingH="fill", sizingV="fill"), foot],
                 gap=0, w=dw, h=dh, x=(W - dw) / 2, y=(H - dh) / 2 + 10, fill=C["surface"], cornerRadius=16,
                 clip=True, effects=SHADOW_LG, absolute=True)
    scrim = R("Háttérsötétítés", W, H, "#0F1424", x=0, y=0, opacity=0.42, absolute=True)
    ed = editor("3 · Nyomtatás", insert_panel())
    ed["children"] += [scrim, dialog]
    return ed


# ---------- home ----------
def template_thumb(kind, w=176, h=104):
    k = []
    if kind == "Árcédula":
        art, lw, lh = price_label(1.9 if h < 70 else (2.4 if h < 100 else 2.8), name="art")
        return F("thumb", [dict(art[0], x=(w - lw) / 2, y=(h - lh) / 2)], w=w, h=h, fill="#EEF1F6", cornerRadius=8,
                 clip=True)
    if kind == "Névkártya":
        k = [R("band", 90, 12, C["primary"], x=0, y=0), T("Kovács Anna", 11, "#111", "Bold", x=8, y=18),
             T("Termékfejlesztés", 7, "#555", x=8, y=34), R("qr", 18, 18, "#111", x=66, y=34)]
        lw, lh = 90, 56
    elif kind == "QR-link":
        k = [R("q1", 34, 34, "none", x=8, y=8, stroke="#111", strokeWeight=4),
             R("q2", 10, 10, "#111", x=20, y=20), T("Menü", 10, "#111", "Bold", x=50, y=12),
             T("labely.app/m", 7, "#555", x=50, y=28)]
        lw, lh = 96, 50
    elif kind == "Leltárcímke":
        k = [T("LELTÁR", 7, "#111", "Bold", x=6, y=5), T("IT-00421", 12, "#111", "ExtraBold", x=6, y=15),
             dict(barcode(56, 14, "bc"), x=6, y=34)]
        lw, lh = 96, 54
    elif kind == "Kábelzászló":
        k = [R("fold", 1, 28, "#BBB", x=47, y=0), T("SW-02/P14", 7, "#111", "Bold", x=5, y=10),
             T("SW-02/P14", 7, "#111", "Bold", x=52, y=10)]
        lw, lh = 94, 28
    elif kind == "Befőttesüveg":
        k = [E("c", 56, 56, "none", x=0, y=0, stroke="#111", strokeWeight=1.5),
             T("Meggy", 11, "#111", "Bold", x=11, y=16), T("2026. július", 6, "#555", x=11, y=32)]
        lw, lh = 56, 56
    elif kind == "Wi-Fi":
        k = [T("Wi-Fi", 11, "#111", "Bold", x=8, y=8), T("OtthonNet", 7, "#555", x=8, y=24),
             R("q", 26, 26, "none", x=60, y=8, stroke="#111", strokeWeight=3)]
        lw, lh = 96, 42
    else:  # Címzés
        k = [T("Feladó", 6, "#555", x=6, y=5), T("Nagy Péter", 10, "#111", "Bold", x=6, y=14),
             T("1051 Budapest, Fő utca 1.", 7, "#333", x=6, y=29)]
        lw, lh = 110, 44
    label = F("art", k, w=lw, h=lh, fill="#FFFFFF", cornerRadius=4, clip=True, effects=SHADOW_SM,
              x=(w - lw) / 2, y=(h - lh) / 2)
    return F("thumb", [label], w=w, h=h, fill="#EEF1F6", cornerRadius=8, clip=True)


def home_sidebar(active="Kezdőlap"):
    nav = [(ic, l, l == active) for ic, l in [("home", "Kezdőlap"), ("templates", "Sablonok"),
           ("clock", "Előzmények"), ("printer", "Nyomtatók"), ("settings", "Beállítások")]]
    nav_nodes = [row("Nav/" + l, [icon(ic, 18, C["primary"] if on else C["text2"]),
                                  T(l, 13, C["primary_d"] if on else C["text"], "SemiBold" if on else "Medium",
                                    name="Label")],
                     gap=10, h=38, padding=[0, 12, 0, 12], cornerRadius=8, fill=C["primary_t"] if on else "none",
                     sizingH="fill") for ic, l, on in nav]
    logo = row("Logó", [
        F("mark", [R("tag", 28, 22, C["primary"], x=2, y=5, cornerRadius=5), E("hole", 6, 6, "#FFFFFF", x=6, y=13),
                   R("l1", 10, 2.5, "#FFFFFF", x=14, y=12, cornerRadius=1), R("l2", 7, 2.5, "#FFFFFFAA", x=14, y=17,
                                                                              cornerRadius=1)], w=32, h=32),
        T("Labely", 18, C["text"], "ExtraBold", letterSpacing="-2%")], gap=10, h=40, padding=[0, 8, 0, 8])
    printer_card = col("Nyomtató kártya", [
        row("h", [row("i", [icon("printer", 18, C["green"])], gap=0, w=34, h=34, mainAlign="center", cornerRadius=8,
                      fill=C["green_t"]),
                  col("t", [T("B21 Pro", 13, C["text"], "Bold"), row("s", [status_dot(C["green"]),
                                                                           T("Csatlakozva · USB", 11, C["muted"])],
                                                                     gap=5)], gap=2, sizingH="fill")], gap=10,
            sizingH="fill"),
        row("bat", [T("Akkumulátor", 11, C["muted"], sizingH="fill"), T("82%", 11, C["text"], "SemiBold")], gap=4,
            sizingH="fill"),
        F("bar", [R("bg", 200, 6, "#E4E7EC", x=0, y=0, cornerRadius=3), R("v", 164, 6, C["green"], x=0, y=0,
                                                                          cornerRadius=3)], w=200, h=6),
        row("pap", [T("Papír", 11, C["muted"], sizingH="fill"), T("50 × 30 mm · fehér", 11, C["text"], "SemiBold")],
            gap=4, sizingH="fill"),
    ], gap=10, padding=14, cornerRadius=12, fill=C["surface"], stroke=C["border"], strokeWeight=1, sizingH="fill")
    sidebar = col("Oldalsáv", [logo, F("sp", h=8, w=1)] + nav_nodes + [F("flex", w=1, sizingV="fill"), printer_card],
                  gap=2, w=248, padding=[16, 12, 16, 12], fill=C["chrome"], sizingV="fill", strokeSides=[0, 1, 0, 0],
                  stroke=C["border"], strokeRightWeight=1)

    return sidebar


def tcard(kind, title, size, selected=False, tw=168, th=80):
    return col("Sablon/" + title, [template_thumb(kind, w=tw, h=th),
                                   col("meta", [T(title, 13, C["text"], "SemiBold"), T(size + " mm", 11, C["muted"])],
                                       gap=2, padding=[0, 4, 0, 4], sizingH="fill")],
               gap=10, padding=8, cornerRadius=12, fill=C["surface"], stroke=C["primary"] if selected else C["border"],
               strokeWeight=2 if selected else 1, sizingH="fill")


def home():
    W, H = 1440, 900
    sidebar = home_sidebar("Kezdőlap")

    def action(ic, title, sub, primary=False):
        fg = "#FFFFFF" if primary else C["text"]
        n = row("Művelet/" + title, [
            row("i", [icon(ic, 22, "#FFFFFF" if primary else C["primary"])], gap=0, w=44, h=44, mainAlign="center",
                cornerRadius=10, fill="#FFFFFF26" if primary else C["primary_t"]),
            col("t", [T(title, 14, fg, "Bold"), T(sub, 12, "#FFFFFFCC" if primary else C["muted"])], gap=3,
                sizingH="fill")],
            gap=14, h=84, padding=[0, 18, 0, 18], cornerRadius=14, sizingH="fill",
            fill=C["primary"] if primary else C["surface"])
        if primary:
            n["props"]["fills"] = [{"type": "linearGradient", "stops": ["#5C7CFA", "#4263EB"]}]
            n["props"]["effects"] = [{"type": "dropShadow", "color": "#4C6EF555", "offset": [0, 8], "radius": 20}]
        else:
            n["props"].update(stroke=C["border"], strokeWeight=1)
        return n

    sizes = [("50 × 30", True), ("40 × 30", False), ("30 × 15", False), ("50 × 50", False), ("40 × 12", False),
             ("70 × 40", False), ("14 × 30 kábel", False)]
    size_row = row("Méretek", [T("Gyors méret:", 12, C["muted"], "Medium")] +
                   [chip(s_.replace(" kábel", "") + " mm" + (" kábel" if "kábel" in s_ else ""), active=on) for s_, on in sizes] + [chip("Egyéni…", ic="plus")], gap=8)

    templates = ["Árcédula", "Névkártya", "Leltárcímke", "QR-link", "Befőttesüveg", "Kábelzászló", "Wi-Fi", "Címzés"]
    tsize = {"Árcédula": "50 × 30", "Névkártya": "70 × 40", "Leltárcímke": "50 × 30", "QR-link": "50 × 30",
             "Befőttesüveg": "50 × 50", "Kábelzászló": "14 × 30", "Wi-Fi": "50 × 30", "Címzés": "70 × 40"}
    cards = [tcard(t, t, tsize[t]) for t in templates]
    grid = col("Sablonrács", [row("r1", cards[:4], gap=14, sizingH="fill"), row("r2", cards[4:], gap=14,
                                                                                  sizingH="fill")],
               gap=14, sizingH="fill")
    own = row("Saját", [tcard("Árcédula", "Akciós ár", "50 × 30"), tcard("Leltárcímke", "Leltár 2026", "50 × 30"),
                        tcard("Befőttesüveg", "Fűszerek", "50 × 50"), tcard("Kábelzászló", "Hálózati kábelek", "14 × 30")],
              gap=14, sizingH="fill")

    recent = [("Árcédula", "ma, 14:02 · 24 rekord", "50 × 30"), ("Kábelzászló", "ma, 11:40", "14 × 30"),
              ("Fűszerek", "tegnap · 12 rekord", "40 × 30"), ("Leltár 2026", "nov. 12.", "50 × 30"),
              ("Költözős dobozok", "nov. 9.", "70 × 40")]
    rec_nodes = [row("Legutóbbi/" + n, [
        row("i", [icon("file", 16, C["primary"])], gap=0, w=34, h=34, mainAlign="center", cornerRadius=8,
            fill=C["primary_t"]),
        col("t", [T(n, 13, C["text"], "SemiBold"), T(d, 11, C["muted"])], gap=2, sizingH="fill"),
        T(s_, 11, C["text2"], "Medium")], gap=10, h=52, padding=[0, 10, 0, 10], cornerRadius=8, sizingH="fill",
        fill="#F7F8FA" if i == 0 else "none") for i, (n, d, s_) in enumerate(recent)]
    rec = col("Legutóbbiak", [row("h", [T("Legutóbbiak", 15, C["text"], "Bold", sizingH="fill"),
                                        T("Összes", 12, C["primary"], "SemiBold")], gap=6, sizingH="fill")] + rec_nodes +
              [F("flex", w=1, sizingV="fill"),
               col("tipp", [row("h", [icon("agent", 16, C["primary"]), T("Ügynökkel is megy", 13, C["text"], "Bold")],
                                gap=8),
                            T("Claude vagy más MCP-kliens a labely mcp paranccsal ugyanazokat a parancsokat használja – "
                              "minden változást élőben látsz.", 12, C["text2"], lineHeight="150%", sizingH="fill")],
                   gap=8, padding=14, cornerRadius=12, fill=C["primary_t"], sizingH="fill")],
              gap=6, w=330, padding=18, cornerRadius=16, fill=C["surface"], stroke=C["border"], strokeWeight=1,
              sizingV="fill")

    main = col("Tartalom", [
        row("Fejléc", [col("t", [T("Szép napot! 👋", 13, C["muted"], "Medium"),
                                 T("Milyen címkét készítünk ma?", 26, C["text"], "Bold", letterSpacing="-1%")], gap=4,
                           sizingH="fill"),
                       field(None, "Sablonok és címkék keresése…", w=320, ic="search", h=40)], gap=16,
            crossAlign="max", sizingH="fill"),
        row("Műveletek", [action("plus", "Új címke", "Üres címke a kiválasztott mérettel", True),
                          action("templates", "Új sablonból", "10 kezdő sablon + saját mappák"),
                          action("data", "Címkék adatból", "CSV vagy Excel: soronként egy címke"),
                          action("open", "Megnyitás…", ".labely fájl a gépről")], gap=14, sizingH="fill"),
        size_row,
        row("Alsó", [col("Sablonok", [row("h", [T("Kezdő sablonok", 15, C["text"], "Bold", sizingH="fill"),
                                               segmented(["Mind", "Bolt", "Iroda", "Otthon"], 0, w=280)], gap=8,
                                          sizingH="fill"), grid,
                                      row("h2", [T("Saját sablonok", 15, C["text"], "Bold", sizingH="fill"), T("Összes", 12, C["primary"], "SemiBold")], gap=6, sizingH="fill"), own], gap=14, sizingH="fill"),
                     rec], gap=20, crossAlign="min", sizingH="fill", sizingV="fill"),
    ], gap=22, padding=[28, 32, 24, 32], sizingH="fill", sizingV="fill")
    body = row("Törzs", [sidebar, main], gap=0, crossAlign="min", sizingH="fill", sizingV="fill")
    return col("1 · Kezdőlap", [titlebar([("home", "Kezdőlap"), ("file", "Árcédula.labely"),
                                          ("file", "Kábelzászló.labely")], 0), body],
               gap=0, w=W, h=H, fill=C["bg"], clip=True, cornerRadius=12)


