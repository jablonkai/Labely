#!/usr/bin/env python3
"""Labely mobile screens (390x844) -> FigCraft spec.  Usage: mobile.py SPEC.json"""
import json, sys
import gen as g
from gen import C, T, F, R, E, row, col, icon, field, chip, segmented, toggle, button, ibtn

W, H = 390, 844
PAD = 16


def flex():
    return F("flex", h=1, sizingH="fill")


def vflex():
    return F("vflex", w=1, sizingV="fill")


# ---------- chrome ----------
def status_bar(dark=False):
    c = "#FFFFFF" if dark else C["text"]
    return row("Állapotsor", [
        T("9:41", 15, c, "SemiBold"), flex(),
        row("ikonok", [F("jel", [R(f"b{i}", 3, 4 + i * 2, c, x=i * 5, y=10 - i * 2, cornerRadius=1) for i in range(4)],
                         w=19, h=12),
                       icon("globe", 15, c, 2), icon("battery", 22, c, 1.8)], gap=6),
    ], gap=0, h=50, padding=[14, 28, 0, 32], sizingH="fill")


def home_indicator():
    return row("Home indikátor", [R("bar", 134, 5, C["text"], cornerRadius=3)], gap=0, h=22, mainAlign="center",
               crossAlign="min", sizingH="fill")


TABS = [("home", "Kezdőlap"), ("templates", "Sablonok"), ("clock", "Előzmények"), ("printer", "Nyomtatók"),
        ("settings", "Beállítások")]


def tab_bar(active):
    items = []
    for ic, l in TABS:
        on = l == active
        items.append(col("Tab/" + l, [icon(ic, 24, C["primary"] if on else C["muted"], 1.8),
                                      T(l, 10, C["primary_d"] if on else C["muted"], "SemiBold" if on else "Medium",
                                        name="Label")],
                         gap=3, crossAlign="center", sizingH="fill", padding=[8, 0, 0, 0]))
    bar = row("Fülsáv", items, gap=0, crossAlign="min", sizingH="fill", h=56)
    return col("Alsó navigáció", [bar, home_indicator()], gap=0, sizingH="fill", fill="#FFFFFFF2",
               strokeSides=[1, 0, 0, 0], stroke=C["border"], strokeTopWeight=1)


def nav_bar(title, left="chev_left", right=None, sub=None):
    t = col("cím", [T(title, 16, C["text"], "Bold", textAlign="center", name="Cím")] +
            ([T(sub, 11, C["muted"], textAlign="center")] if sub else []), gap=1, crossAlign="center", sizingH="fill")
    return row("Navigációs sáv", [row("bal", [ibtn(left, size=36)] if left else [], gap=0, w=84),
                                   t, row("jobb", right or [], gap=2, w=84, mainAlign="max")],
               gap=0, h=48, padding=[0, 8, 0, 8], sizingH="fill")


def screen(name, kids, bg=None, tab=None, gap=0):
    body = col("Tartalom", kids, gap=16, padding=[4, PAD, 16, PAD], sizingH="fill", sizingV="fill", clip=True) \
        if tab is not None else None
    if tab is not None:
        children = [status_bar(), body, tab_bar(tab)]
    else:
        children = kids
    return col(name, children, gap=gap, w=W, h=H, fill=bg or C["bg"], clip=True, cornerRadius=44)


def large_title(title, sub=None, right=None):
    return row("Fejléc", [col("t", [T(title, 30, C["text"], "Bold", letterSpacing="-2%", name="Cím")] +
                                   ([T(sub, 13, C["muted"], lineHeight="140%", sizingH="fill")] if sub else []),
                              gap=4, sizingH="fill")] + (right or []), gap=8, crossAlign="max", sizingH="fill")


def search(ph):
    return row("Keresés", [icon("search", 18, C["muted"]), T(ph, 15, C["muted"], sizingH="fill")], gap=8, h=42,
               padding=[0, 12, 0, 12], cornerRadius=12, fill="#E9ECF1", sizingH="fill")


def card(name, kids, **p):
    p.setdefault("sizingH", "fill")
    p.setdefault("gap", 0)
    return col(name, kids, cornerRadius=16, fill=C["surface"], stroke=C["border"], strokeWeight=1, clip=True, **p)


def list_row(title, sub=None, lead=None, trail=None, last=False, h=None, tint=None):
    kids = []
    if lead:
        kids.append(row("i", [icon(lead, 18, tint or C["primary"])], gap=0, w=34, h=34, mainAlign="center",
                        cornerRadius=9, fill=C["primary_t"] if not tint else tint + "1F"))
    kids.append(col("t", [T(title, 15, C["text"], "SemiBold", name="Cím")] +
                    ([T(sub, 12, C["muted"], name="Al", sizingH="fill")] if sub else []), gap=2, sizingH="fill"))
    if trail is None:
        trail = [icon("chev_right", 16, "#B8BFCC")]
    kids += trail
    p = dict(gap=12, padding=[12, 14, 12, 14], sizingH="fill")
    if h:
        p["h"] = h
    if not last:
        p.update(strokeSides=[0, 0, 1, 0], stroke=C["border_s"], strokeBottomWeight=1)
    return row("Sor/" + title, kids, **p)


def section_title(t, action=None):
    return row("Szakasz/" + t, [T(t, 17, C["text"], "Bold", sizingH="fill")] +
               ([T(action, 14, C["primary"], "SemiBold")] if action else []), gap=6, sizingH="fill")


def pill(text, color, bg, ic=None):
    return row("Pill/" + text, ([icon(ic, 12, color, 2)] if ic else []) + [T(text, 11, color, "SemiBold")], gap=3,
               h=22, padding=[0, 8, 0, 8], cornerRadius=11, fill=bg)


def big_button(label, kind="primary", ic=None, name=None):
    b = button(label, kind, ic, h=50, name=name)
    b["props"]["sizingH"] = "fill"
    b["props"]["cornerRadius"] = 14
    for k in b["children"]:
        if k.get("type") == "text":
            k["props"]["fontSize"] = 16
    return b


def sheet(name, title, kids, foot=None, h=None, handle=True):
    head = row("Fej", [T(title, 18, C["text"], "Bold", sizingH="fill"),
                       row("x", [icon("close", 16, C["text2"], 2)], gap=0, w=30, h=30, mainAlign="center",
                           cornerRadius=15, fill="#EEF0F4")], gap=8, padding=[4, 20, 8, 20], sizingH="fill")
    grab = row("Fogantyú", [R("g", 36, 5, "#D0D5DD", cornerRadius=3)], gap=0, h=18, mainAlign="center",
               crossAlign="max", sizingH="fill")
    body = col("Törzs", kids, gap=16, padding=[8, 20, 8, 20], sizingH="fill", sizingV="fill" if h else "hug")
    parts = ([grab] if handle else []) + [head, body]
    if foot:
        parts.append(col("Lábléc", [row("gombok", foot, gap=10, sizingH="fill"), home_indicator()], gap=6,
                         padding=[12, 20, 0, 20], sizingH="fill"))
    else:
        parts.append(home_indicator())
    s = col(name, parts, gap=0, w=W, h=h, fill=C["surface"], cornerRadius=24, effects=g.SHADOW_LG, absolute=True,
            x=0, y=(H - h) if h else None, clip=True)
    return s


def overlay(base, sh):
    base["children"] += [R("Háttérsötétítés", W, H, "#0F1424", x=0, y=0, opacity=0.4, absolute=True), sh]
    return base


def label_art(scale, selected=None, name="Címke"):
    art, lw, lh = g.price_label(scale, selected=selected, name=name)
    return art, lw, lh


def canvas(h, sel=None, scale=6.4, chip_txt="50 × 30 mm · Fehér · Résközös", extra=None):
    art, lw, lh = label_art(scale, selected=sel)
    lx, ly = (W - lw) / 2, (h - lh) / 2 - 18
    for a in art:
        a["x"] = lx + a.get("x", 0)
        a["y"] = ly + a.get("y", 0)
    info = row("Címkeinfó", [icon("rect", 13, C["text2"]), T(chip_txt, 12, C["text"], "SemiBold"),
                             icon("chev_down", 12, C["muted"])], gap=6, h=30, padding=[0, 12, 0, 12],
               cornerRadius=15, fill=C["surface"], effects=g.SHADOW_SM, x=(W - 250) / 2, y=ly + lh + 18)
    zoom = row("Nagyítás", [T("128%", 12, C["text2"], "SemiBold")], gap=0, h=26, padding=[0, 10, 0, 10],
               cornerRadius=13, fill="#FFFFFFCC", x=W - 70, y=12)
    return F("Vászon", art + [info, zoom] + (extra or []), w=W, h=h, fill=C["canvas"], clip=True, sizingH="fill")


def editor_top(title="Árcédula", sub="Mentve"):
    return row("Szerkesztő fejléc", [
        ibtn("chev_left", size=36),
        col("t", [row("n", [T(title, 16, C["text"], "Bold"), icon("chev_down", 14, C["muted"])], gap=4),
                  T(sub, 11, C["muted"])], gap=1, sizingH="fill", crossAlign="center"),
        ibtn("undo", size=36), ibtn("printer", size=36, color=C["primary"]),
    ], gap=2, h=50, padding=[0, 8, 0, 8], sizingH="fill")


def editor_tools(active=None):
    tools = [("plus", "Beszúrás"), ("layers", "Rétegek"), ("data", "Adatok"), ("templates", "Sablonok"),
             ("settings", "Címke")]
    items = [col("Eszköz/" + l, [row("i", [icon(ic, 22, C["primary"] if l == active else C["text2"], 1.8)], gap=0,
                                     w=44, h=32, mainAlign="center", cornerRadius=10,
                                     fill=C["primary_t"] if l == active else "none"),
                                 T(l, 10, C["primary_d"] if l == active else C["text2"],
                                   "SemiBold" if l == active else "Medium")],
                 gap=2, crossAlign="center", sizingH="fill") for ic, l in tools]
    return col("Eszközsáv", [row("ikonok", items, gap=0, padding=[8, 4, 4, 4], sizingH="fill"), home_indicator()],
               gap=0, sizingH="fill", fill=C["surface"], strokeSides=[1, 0, 0, 0], stroke=C["border"],
               strokeTopWeight=1)


def editor_shell(name, canvas_h, middle, active=None, sel=(2.4, 13.6, 24.5, 9.6)):
    return col(name, [status_bar(), editor_top(), canvas(canvas_h, sel=sel)] + middle + [vflex(), editor_tools(active)],
               gap=0, w=W, h=H, fill=C["surface"], clip=True, cornerRadius=44)


def ctx_bar(kids):
    return row("Kontextus sáv", kids, gap=6, h=52, padding=[0, 12, 0, 12], sizingH="fill",
               strokeSides=[1, 0, 0, 0], stroke=C["border"], strokeTopWeight=1)


# ---------- 1 · Kezdőlap ----------
def m_home():
    def action(ic, t, sub, primary=False):
        fg = "#FFFFFF" if primary else C["text"]
        n = col("Művelet/" + t, [
            row("i", [icon(ic, 20, "#FFFFFF" if primary else C["primary"])], gap=0, w=38, h=38, mainAlign="center",
                cornerRadius=10, fill="#FFFFFF26" if primary else C["primary_t"]),
            col("t", [T(t, 14, fg, "Bold"), T(sub, 11, "#FFFFFFCC" if primary else C["muted"], sizingH="fill")],
                gap=2, sizingH="fill")], gap=10, padding=14, cornerRadius=16, sizingH="fill", h=116)
        if primary:
            n["props"]["fills"] = [{"type": "linearGradient", "stops": ["#5C7CFA", "#4263EB"]}]
            n["props"]["effects"] = [{"type": "dropShadow", "color": "#4C6EF555", "offset": [0, 8], "radius": 20}]
        else:
            n["props"].update(fill=C["surface"], stroke=C["border"], strokeWeight=1)
        return n

    sizes = ["50 × 30 mm", "40 × 30 mm", "30 × 15 mm", "50 × 50 mm", "Egyéni…"]
    sz = row("Gyors méretek", [chip(s, active=i == 0, ic="plus" if s.startswith("Egy") else None)
                               for i, s in enumerate(sizes)], gap=8)
    temps = [("Árcédula", "50 × 30"), ("Névkártya", "70 × 40"), ("Leltárcímke", "50 × 30"), ("QR-link", "50 × 30")]
    tc = [g.tcard(k, k, s, tw=154, th=78) for k, s in temps]
    grid = col("Sablonrács", [row("r1", tc[:2], gap=12, sizingH="fill"), row("r2", tc[2:], gap=12, sizingH="fill")],
               gap=12, sizingH="fill")
    recent = card("Legutóbbiak", [
        list_row("Árcédula", "ma, 14:02 · 24 rekord", "file"),
        list_row("Kábelzászló", "ma, 11:40 · 14 × 30 mm", "file"),
        list_row("Fűszerek", "tegnap · 12 rekord", "file", last=True)])
    printer = row("Nyomtató", [g.status_dot(C["green"]), T("B21 Pro", 12, C["text"], "SemiBold"),
                               T("· 82%", 12, C["muted"])], gap=5, h=30, padding=[0, 10, 0, 10], cornerRadius=15,
                  fill=C["green_t"])
    kids = [
        row("Felső", [row("Logó", [F("mark", [R("tag", 26, 20, C["primary"], x=1, y=5, cornerRadius=5),
                                               E("hole", 5, 5, "#FFFFFF", x=5, y=12.5),
                                               R("l1", 9, 2.5, "#FFFFFF", x=13, y=11, cornerRadius=1)], w=30, h=30),
                                    T("Labely", 20, C["text"], "ExtraBold", letterSpacing="-2%")], gap=8),
                      flex(), printer], gap=8, sizingH="fill"),
        col("Üdv", [T("Szép napot! 👋", 13, C["muted"], "Medium"),
                    T("Milyen címkét készítünk ma?", 24, C["text"], "Bold", letterSpacing="-1%", sizingH="fill",
                      lineHeight="120%")], gap=4, sizingH="fill"),
        search("Sablonok és címkék keresése"),
        row("Műveletek", [action("plus", "Új címke", "50 × 30 mm, üres", True),
                          action("templates", "Sablonból", "10 kezdő sablon"),
                          action("data", "Adatból", "CSV / Excel")], gap=10, sizingH="fill"),
        sz,
        section_title("Kezdő sablonok", "Összes"), grid,
        section_title("Legutóbbiak", "Összes"), recent,
    ]
    return screen("M1 · Kezdőlap", kids, tab="Kezdőlap")


# ---------- 2 · Szerkesztő ----------
def m_editor():
    fmt = ctx_bar([
        row("Betű", [T("Inter · ExtraBold", 13, C["text"], "SemiBold"), icon("chev_down", 12, C["muted"])], gap=4,
            h=34, padding=[0, 10, 0, 10], cornerRadius=8, fill="#F2F4F7"),
        row("Méret", [icon("minus", 14, C["text2"]), T("32", 13, C["text"], "SemiBold"), icon("plus", 14, C["text2"])],
            gap=8, h=34, padding=[0, 10, 0, 10], cornerRadius=8, fill="#F2F4F7"),
        flex(), ibtn("bold", True, size=34), ibtn("t_left", size=34), ibtn("more", size=34)])
    sel = row("Kijelölés", [row("i", [icon("text", 16, C["primary"])], gap=0, w=30, h=30, mainAlign="center",
                                 cornerRadius=8, fill=C["primary_t"]),
                             col("t", [T("Ár", 14, C["text"], "Bold"), T("Szöveg · kötve: Ár oszlop", 11, C["muted"])],
                                 gap=1, sizingH="fill"),
                             ibtn("copy", size=34), ibtn("trash", size=34, color=C["red"])],
              gap=10, h=56, padding=[0, 12, 0, 16], sizingH="fill", strokeSides=[1, 0, 0, 0], stroke=C["border"],
              strokeTopWeight=1)
    props = row("Pozíció", [field("X", "2,4", suffix="mm"), field("Y", "13,6", suffix="mm"),
                            field("Sz", "24,5", suffix="mm")], gap=8, padding=[10, 16, 12, 16], sizingH="fill")
    return editor_shell("M2 · Szerkesztő", 446, [sel, props, fmt], active=None)


# ---------- 3 · Nyomtatás ----------
def m_print():
    base = m_editor(); base["name"] = "M3 · Nyomtatás"
    art, lw, lh = label_art(4.2, name="Előnézet")
    prev = F("Előnézet", [dict(art[0], x=(350 - lw) / 2, y=18)] +
             [row("lapozó", [icon("chev_left", 14, C["text2"]), T("1 / 24 rekord", 12, C["text2"], "SemiBold"),
                             icon("chev_right", 14, C["text2"])], gap=10, x=(350 - 140) / 2, y=18 + lh + 10)],
             w=350, h=lh + 64, fill="#EEF1F6", cornerRadius=14, clip=True, sizingH="fill")
    printer = row("Nyomtató", [row("i", [icon("printer", 18, C["green"])], gap=0, w=36, h=36, mainAlign="center",
                                    cornerRadius=9, fill=C["green_t"]),
                                col("t", [T("NIIMBOT B21 Pro", 14, C["text"], "Bold"),
                                          row("s", [g.status_dot(C["green"]), T("USB · 82% · 50 × 30 mm", 11, C["muted"])],
                                              gap=5)], gap=2, sizingH="fill"), icon("chev_down", 16, C["muted"])],
                  gap=10, padding=12, cornerRadius=12, stroke=C["primary"], strokeWeight=1.5, sizingH="fill")
    stepper = row("Léptető", [ibtn("minus", size=32), T("1", 15, C["text"], "Bold"), ibtn("plus", size=32)], gap=10,
                  h=40, padding=[0, 4, 0, 4], cornerRadius=10, fill="#F2F4F7")
    opts = card("Beállítások", [
        list_row("Példányszám", None, trail=[stepper]),
        list_row("Rekordok", "Mind a 24 rekord", trail=[T("1–24", 14, C["text2"], "SemiBold"),
                                                       icon("chev_right", 16, "#B8BFCC")]),
        list_row("Sötétség", None, trail=[g.slider(120, 0.6) if hasattr(g, "slider") else
                                          F("sl", [R("t", 120, 4, "#E4E7EC", x=0, y=8, cornerRadius=2),
                                                   R("f", 72, 4, C["primary"], x=0, y=8, cornerRadius=2),
                                                   E("k", 20, 20, "#FFFFFF", x=62, y=0, stroke=C["primary"],
                                                     strokeWeight=2)], w=120, h=20),
                                          T("3 / 5", 13, C["text2"], "SemiBold")], last=True)])
    ok = row("Ellenőrzés", [icon("check", 16, C["green"], 2), T("Ellenőrizve, nincs hiba", 13, C["green"], "SemiBold",
                                                                 sizingH="fill"),
                            T("24 címke", 13, C["text"], "Bold")], gap=8, h=40, padding=[0, 12, 0, 12],
             cornerRadius=10, fill=C["green_t"], sizingH="fill")
    sh = sheet("Lap/Nyomtatás", "Nyomtatás", [prev, printer, opts, ok],
               foot=[big_button("24 címke nyomtatása", ic="printer")], h=720)
    return overlay(base, sh)


# ---------- 4 · Adatok ----------
def m_data():
    recs = [("1", "Bio alma", "649", True), ("2", "Körte", "899", False), ("3", "Banán", "749", False),
            ("4", "Szilva", "1299", False)]
    rows_ = [row("Rekord/" + n, [T(i, 12, C["muted"], "SemiBold"),
                                 col("t", [T(n, 14, C["text"], "SemiBold"), T("Gyümölcs · 599800123456" + i, 11,
                                                                              C["muted"])], gap=1, sizingH="fill"),
                                 T(p + " Ft", 14, C["text"], "Bold")],
                 gap=10, h=52, padding=[0, 16, 0, 16], sizingH="fill",
                 fill=C["primary_t"] if on else "none", strokeSides=[0, 0, 1, 0], stroke=C["border_s"],
                 strokeBottomWeight=1) for i, n, p, on in recs]
    src = row("Forrás", [row("i", [icon("file", 16, C["green"])], gap=0, w=30, h=30, mainAlign="center",
                              cornerRadius=8, fill=C["green_t"]),
                         col("t", [T("termekek.csv", 13, C["text"], "SemiBold"),
                                   T("24 rekord · 6 oszlop · 4 kötve", 11, C["muted"])], gap=1, sizingH="fill"),
                         row("lap", [ibtn("chev_left", size=30), T("1 / 24", 12, C["text"], "Bold"),
                                     ibtn("chev_right", size=30)], gap=2)],
              gap=10, h=56, padding=[0, 10, 0, 16], sizingH="fill", strokeSides=[1, 0, 1, 0], stroke=C["border"],
              strokeTopWeight=1, strokeBottomWeight=1)
    return editor_shell("M4 · Adatok és rekordok", 300, [src, col("Rekordok", rows_, gap=0, sizingH="fill",
                                                                     sizingV="fill", clip=True)],
                        active="Adatok", sel=None)


# ---------- 5 · Rétegek ----------
def m_layers():
    base = editor_shell("M5 · Rétegek", 446, [vflex()], active="Rétegek", sel=(2.4, 2.2, 28, 7.4))
    layers = [("text", "Termék", True), ("text", "Leírás", False), ("rect", "Elválasztó", False),
              ("text", "Ár", False), ("text", "Egység", False), ("barcode", "Vonalkód", False),
              ("text", "EAN", False), ("date", "Dátum", False)]
    rows_ = [row("Réteg/" + n, [icon("chev_right", 12, "#C3C9D4"), icon(ic, 16, C["primary"] if on else C["text2"]),
                                T(n, 14, C["primary_d"] if on else C["text"], "SemiBold" if on else "Medium",
                                  sizingH="fill"),
                                pill("Ár oszlop", C["primary_d"], C["primary_t"], "link") if n in ("Termék", "Ár") else
                                (pill("Kívül esik", "#B35C00", C["amber_t"], "warn") if n == "Dátum" else flex()),
                                icon("eye", 16, C["muted"]), icon("lock" if n == "Elválasztó" else "unlock", 16,
                                                                  C["muted"])],
                 gap=10, h=44, padding=[0, 12, 0, 8], cornerRadius=10, sizingH="fill",
                 fill=C["primary_t"] if on else "none") for ic, n, on in layers]
    tools = row("Rendezés", [ibtn(i, size=36) for i in ("to_front", "up", "down", "to_back", "group", "copy")] +
                [flex(), ibtn("trash", size=36, color=C["red"])], gap=2, sizingH="fill")
    sh = sheet("Lap/Rétegek", "Rétegek", [col("lista", rows_, gap=2, sizingH="fill"), tools], h=560)
    return overlay(base, sh)


# ---------- 6 · Vonalkód tulajdonságok ----------
def m_barcode():
    base = editor_shell("M6 · Vonalkód", 446, [vflex()], active=None, sel=(28.4, 15.8, 19.2, 11.2))
    typ = segmented(["EAN-13", "Code 128", "QR", "DataMatrix"], 0)
    val = col("Érték", [T("Érték", 12, C["text2"], "SemiBold"),
                        field(None, "5998001234567", h=44, ic="link"),
                        row("ok", [icon("check", 13, C["green"], 2),
                                   T("Érvényes · ellenőrző számjegy: 7", 12, C["green"], "Medium")], gap=5)], gap=8,
              sizingH="fill")
    opts = card("Opciók", [list_row("Olvasható szöveg", "Számok a vonalkód alatt", trail=[toggle(True)]),
                           list_row("Kötés", "EAN oszlop · termekek.csv", "link", trail=None),
                           list_row("Szöveg helye", None, trail=[segmented(["Alul", "Felül"], 0, w=130)],
                                    last=True)])
    sh = sheet("Lap/Vonalkód", "Vonalkód", [typ, val, opts], h=470)
    return overlay(base, sh)


# ---------- 7 · Sablonkönyvtár ----------
def m_library():
    cats = row("Kategóriák", [chip("Mind", True), chip("Bolt"), chip("Iroda"), chip("Otthon"), chip("Saját")], gap=8)
    temps = [("Árcédula", "Akciós ár", "50 × 30"), ("Árcédula", "Polccímke", "50 × 30"),
             ("Leltárcímke", "Raktári kód", "50 × 30"), ("QR-link", "Étlap QR", "50 × 30"),
             ("Névkártya", "Csomagcímke", "70 × 40"), ("Befőttesüveg", "Házi lekvár", "50 × 50"),
             ("Wi-Fi", "Vendég Wi-Fi", "50 × 30"), ("Kábelzászló", "Kasszakábel", "14 × 30")]
    tc = [g.tcard(k, t, s, selected=i == 0, tw=154, th=88) for i, (k, t, s) in enumerate(temps)]
    grid = col("Sablonrács", [row(f"r{i}", tc[i:i + 2], gap=12, sizingH="fill") for i in range(0, 8, 2)], gap=12,
               sizingH="fill")
    kids = [large_title("Sablonok", "Kezdő sablonok és saját mappák",
                        right=[row("plus", [icon("plus", 20, "#FFFFFF", 2)], gap=0, w=40, h=40, mainAlign="center",
                                   cornerRadius=20, fill=C["primary"])]),
            search("Keresés a sablonok között"), cats,
            row("h", [T("Bolt · 12 sablon", 13, C["muted"], "SemiBold", sizingH="fill"),
                      icon("grid", 18, C["text2"])], gap=6, sizingH="fill"), grid]
    return screen("M7 · Sablonkönyvtár", kids, tab="Sablonok")


# ---------- 8 · Nyomtatók ----------
def m_printers():
    hero = card("Aktív nyomtató", [
        row("h", [row("img", [icon("printer", 30, C["primary"], 1.6)], gap=0, w=64, h=64, mainAlign="center",
                      cornerRadius=14, fill=C["primary_t"]),
                  col("t", [T("NIIMBOT B21 Pro", 17, C["text"], "Bold"),
                            T("B21P-4823-1193 · Firmware 1.4.2", 12, C["muted"]),
                            row("pills", [pill("Csatlakozva", C["green"], C["green_t"], "check"),
                                          pill("USB", C["text2"], "#EEF0F4", "usb")], gap=6)], gap=4,
                      sizingH="fill")], gap=14, padding=16, sizingH="fill"),
        row("stats", [col("s/" + k, [T(k, 11, C["muted"], "Medium"), T(v, 17, C["text"], "Bold")], gap=3,
                          sizingH="fill", padding=[12, 0, 12, 14], strokeSides=[0, 1, 0, 0] if i < 2 else [0, 0, 0, 0],
                          stroke=C["border_s"], strokeRightWeight=1)
                      for i, (k, v) in enumerate([("Akku", "82%"), ("Papír", "50×30"), ("Ma", "1 284")])],
            gap=0, sizingH="fill", strokeSides=[1, 0, 0, 0], stroke=C["border_s"], strokeTopWeight=1),
        row("akciók", [button("Tesztcímke", "secondary", "printer", h=40), button("Kalibrálás", "secondary", "scan",
                                                                                  h=40)],
            gap=10, padding=[0, 16, 16, 16], sizingH="fill"),
    ])
    for b in hero["children"][2]["children"]:
        b["props"]["sizingH"] = "fill"
    known = card("Ismert", [
        list_row("NIIMBOT D110", "Bluetooth · utoljára tegnap", "bluetooth", trail=[T("Csatlakozás", 13, C["primary"],
                                                                                       "SemiBold")]),
        list_row("NIIMBOT B1", "Bluetooth · nov. 9.", "bluetooth", trail=[T("Csatlakozás", 13, C["primary"],
                                                                             "SemiBold")], last=True)])
    defaults = card("Alapok", [
        list_row("Papírtípus", "Résközös címke", trail=[icon("chev_right", 16, "#B8BFCC")]),
        list_row("Alapértelmezett sötétség", "3 / 5"),
        list_row("Automatikus kikapcsolás", "15 perc után", last=True)])
    kids = [large_title("Nyomtatók", "USB vagy Bluetooth hőnyomtatók",
                        right=[row("scan", [icon("search", 18, C["primary"])], gap=0, w=40, h=40, mainAlign="center",
                                   cornerRadius=20, fill=C["primary_t"])]),
            hero, section_title("Ismert nyomtatók", "Keresés"), known, section_title("Nyomtatási beállítások"),
            defaults]
    return screen("M8 · Nyomtatók", kids, tab="Nyomtatók")


# ---------- 9 · Előzmények ----------
def m_history():
    running = col("Folyamatban", [
        row("h", [icon("printer", 16, C["primary"]), T("Nyomtatás folyamatban", 14, C["text"], "Bold", sizingH="fill"),
                  T("16 / 24", 13, C["primary_d"], "Bold")], gap=8, sizingH="fill"),
        F("bar", [R("bg", 326, 6, C["primary_t2"], x=0, y=0, cornerRadius=3),
                  R("v", 217, 6, C["primary"], x=0, y=0, cornerRadius=3)], w=326, h=6, sizingH="fill"),
        row("f", [T("Árcédula · B21 Pro · kb. 20 mp", 12, C["text2"], sizingH="fill"),
                  T("Szünet", 13, C["primary"], "SemiBold")], gap=8, sizingH="fill")],
        gap=10, padding=14, cornerRadius=14, fill=C["primary_t"], sizingH="fill")
    items = [("Árcédula", "ma, 14:02 · B21 Pro", "24", "ok"), ("Kábelzászló", "ma, 11:40 · B21 Pro", "8", "ok"),
             ("Fűszerek", "tegnap, 18:21 · D110", "12", "ok"), ("Leltár 2026", "nov. 12. · B21 Pro", "32 / 50", "part"),
             ("Költözős dobozok", "nov. 9. · B21 Pro", "18", "ok"), ("Csomagcímke", "nov. 3. · B21 Pro", "0 / 9", "err")]
    st = {"ok": pill("Kész", C["green"], C["green_t"], "check"),
          "part": pill("Megszakítva", "#B35C00", C["amber_t"]),
          "err": pill("Papír elfogyott", C["red"], "#FFF0F0", "warn")}
    rows_ = [list_row(n, d + " · " + c + " címke", "file",
                      trail=[json.loads(json.dumps(st[s])), icon("more", 16, C["muted"])],
                      last=i == len(items) - 1) for i, (n, d, c, s) in enumerate(items)]
    kids = [large_title("Előzmények", "Minden nyomtatás a gépen tárolva"),
            row("Szűrők", [chip("Mind", True), chip("Ma"), chip("7 nap"), chip("B21 Pro", ic="printer")], gap=8),
            running, section_title("Ezen a héten", "Törlés"), card("Lista", rows_)]
    return screen("M9 · Előzmények", kids, tab="Előzmények")


# ---------- 10 · Beállítások ----------
def m_settings():
    gen_ = card("Általános", [
        list_row("Nyelv", None, "globe", trail=[T("Magyar", 14, C["muted"]), icon("chev_right", 16, "#B8BFCC")]),
        list_row("Mértékegység", None, "line", trail=[segmented(["mm", "hüvelyk"], 0, w=130)]),
        list_row("Automatikus mentés", None, "download", trail=[toggle(True)], last=True)])
    look = card("Megjelenés", [
        list_row("Téma", None, "sun", trail=[segmented(["Világos", "Sötét", "Rend."], 0, w=170)]),
        list_row("Vonalzók megjelenítése", None, "grid", trail=[toggle(True)], last=True)])
    agent = card("Ügynökök", [
        list_row("MCP-szerver", "127.0.0.1:7878/mcp", "agent", trail=[toggle(True)]),
        list_row("Hozzáférési token", "••••••••••3f9a", "key", trail=[icon("copy", 16, C["muted"])]),
        list_row("Ügynöki módosítások jóváhagyása", None, "check", trail=[toggle(False)], last=True)])
    about = card("Névjegy", [list_row("Labely 0.3.0", "Ellenőrizve ma", "info", trail=[T("Dokumentáció", 13,
                                                                                         C["primary"], "SemiBold")],
                                      last=True)])
    kids = [large_title("Beállítások"), search("Beállítás keresése"),
            T("ÁLTALÁNOS", 12, C["muted"], "SemiBold", letterSpacing="4%"), gen_,
            T("MEGJELENÉS", 12, C["muted"], "SemiBold", letterSpacing="4%"), look,
            T("ÜGYNÖKÖK (MCP)", 12, C["muted"], "SemiBold", letterSpacing="4%"), agent, about]
    return screen("M10 · Beállítások", kids, tab="Beállítások")


# ---------- 11 · Új címke ----------
def m_new_label():
    base = m_home(); base["name"] = "M11 · Új címke"
    sizes = ["50 × 30", "40 × 30", "30 × 15", "50 × 50", "40 × 12", "70 × 40"]
    sz = col("Méretek", [row(f"r{i}", [row("Méret/" + s, [T(s + " mm", 13, C["primary_d"] if s == "50 × 30" else
                                                         C["text"], "SemiBold")], gap=0, h=44, mainAlign="center",
                                          cornerRadius=10, sizingH="fill",
                                          fill=C["primary_t"] if s == "50 × 30" else C["surface"],
                                          stroke=C["primary"] if s == "50 × 30" else C["border"],
                                          strokeWeight=1.5 if s == "50 × 30" else 1)
                                      for s in sizes[i:i + 3]], gap=8, sizingH="fill") for i in (0, 3)], gap=8,
             sizingH="fill")
    prev = F("Előnézet", [R("papír", 160, 96, "#FFFFFF", x=95, y=22, cornerRadius=6, effects=g.SHADOW_SM),
                          T("50 mm", 11, C["muted"], "Medium", x=158, y=4), T("30 mm", 11, C["muted"], "Medium",
                                                                              x=262, y=62)],
             w=350, h=136, fill="#EEF1F6", cornerRadius=14, sizingH="fill")
    custom = row("Egyéni", [field("Szélesség", "50", suffix="mm", h=44), field("Magasság", "30", suffix="mm", h=44)],
                 gap=10, sizingH="fill")
    opts = card("Opciók", [list_row("Papírtípus", None, trail=[T("Résközös", 14, C["muted"]),
                                                               icon("chev_right", 16, "#B8BFCC")]),
                           list_row("Tájolás", None, trail=[segmented(["Fekvő", "Álló"], 0, w=140)]),
                           list_row("Címke színe", None, trail=[row("sw", [E(c, 20, 20, c, stroke="#C3C9D4",
                                                                            strokeWeight=1)
                                                                          for c in ("#FFFFFF", "#FFE066", "#FFB3C7",
                                                                                    "#A5D8FF")], gap=6)], last=True)])
    fit = row("Illik", [icon("check", 14, C["green"], 2), T("Illik a B21 Pro nyomtatóhoz", 12, C["green"], "Medium")],
              gap=6)
    sh = sheet("Lap/Új címke", "Új címke", [prev, T("Méret", 13, C["text2"], "SemiBold"), sz, custom, opts, fit],
               foot=[big_button("Mégse", "secondary"), big_button("Létrehozás")], h=780)
    return overlay(base, sh)


# ---------- 12 · Exportálás ----------
def m_export():
    base = m_editor(); base["name"] = "M12 · Exportálás"
    fmt = row("Formátumok", [col("F/" + f, [icon(ic, 22, C["primary"] if i == 0 else C["text2"]),
                                             T(f, 13, C["primary_d"] if i == 0 else C["text"], "SemiBold")],
                                  gap=6, crossAlign="center", padding=[14, 0, 12, 0], cornerRadius=12, sizingH="fill",
                                  fill=C["primary_t"] if i == 0 else C["surface"],
                                  stroke=C["primary"] if i == 0 else C["border"], strokeWeight=1.5 if i == 0 else 1)
                              for i, (f, ic) in enumerate([("PNG", "image"), ("PDF", "file"), ("SVG", "pencil")])],
              gap=10, sizingH="fill")
    opts = card("Opciók", [
        list_row("Felbontás", None, trail=[segmented(["203", "300", "600 dpi"], 1, w=170)]),
        list_row("Rekordok", None, trail=[segmented(["Aktuális", "Mind (24)"], 1, w=170)]),
        list_row("Átlátszó háttér", None, trail=[toggle(True)]),
        list_row("Nyomtatható kép (1 bites)", None, trail=[toggle(False)], last=True)])
    dest = col("Cél", [T("Mentés helye", 13, C["text2"], "SemiBold"),
                       field(None, "Fájlok › Címkék › Árcédula-{rekord}.png", h=44, ic="folder"),
                       T("24 fájl · körülbelül 1,8 MB", 12, C["muted"])], gap=8, sizingH="fill")
    sh = sheet("Lap/Exportálás", "Exportálás", [fmt, opts, dest],
               foot=[big_button("Megosztás", "secondary", "upload"), big_button("Exportálás", ic="download")], h=600)
    return overlay(base, sh)


# ---------- build ----------
frames = [m_home(), m_editor(), m_print(), m_data(), m_layers(), m_barcode(), m_library(), m_printers(),
          m_history(), m_settings(), m_new_label(), m_export()]
for i, f in enumerate(frames):
    f["x"] = (i % 6) * 470
    f["y"] = (i // 6) * 940

spec = {
    "styles": {"paint": {"Labely/Primary": C["primary"], "Labely/Primary tint": C["primary_t"],
                         "Labely/Text": C["text"], "Labely/Muted": C["muted"], "Labely/Border": C["border"],
                         "Labely/Canvas": C["canvas"], "Labely/Background": C["bg"], "Labely/Success": C["green"],
                         "Labely/Warning": C["amber"], "Labely/Danger": C["red"]},
               "text": {"Mobile/Large title": {"fontFamily": "Inter", "fontStyle": "Bold", "fontSize": 30},
                        "Mobile/Title": {"fontFamily": "Inter", "fontStyle": "Bold", "fontSize": 18},
                        "Mobile/Headline": {"fontFamily": "Inter", "fontStyle": "SemiBold", "fontSize": 15},
                        "Mobile/Body": {"fontFamily": "Inter", "fontStyle": "Regular", "fontSize": 15},
                        "Mobile/Caption": {"fontFamily": "Inter", "fontStyle": "Medium", "fontSize": 11}}},
    "frames": frames,
    "save": "labely-mobile.figcraft",
    "exports": [{"frames": "each", "dir": "png-mobile", "format": "png", "scale": 2}],
    "preview": "preview-mobile.png",
}
json.dump(spec, open(sys.argv[1], "w"), ensure_ascii=False, indent=1)
print("ok", len(frames))
