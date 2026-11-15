#!/usr/bin/env python3
"""All Labely desktop screens -> FigCraft spec.  Usage: screens.py SPEC.json"""
import json, sys
import gen as g
from gen import C, T, F, R, E, row, col, icon, field, section, chip, segmented, toggle, button, ibtn

W, H = 1440, 900
TABS = [("home", "Kezdőlap"), ("file", "Árcédula.labely"), ("file", "Kábelzászló.labely")]


def lab(t):
    return T(t, 12, C["text2"], "SemiBold")


def slider(w, frac, name="slider"):
    return F(name, [R("track", w, 4, "#E4E7EC", x=0, y=8, cornerRadius=2),
                    R("fill", w * frac, 4, C["primary"], x=0, y=8, cornerRadius=2),
                    E("knob", 20, 20, "#FFFFFF", x=w * frac - 10, y=0, stroke=C["primary"], strokeWeight=2,
                      effects=g.SHADOW_SM)], w=w, h=20)


def swatches(active=0):
    cols = [("Fehér", "#FFFFFF"), ("Sárga", "#FFE066"), ("Rózsaszín", "#FFB3C7"), ("Kék", "#A5D8FF"),
            ("Zöld", "#B2F2BB"), ("Átlátszó", None)]
    kids = []
    for i, (n, c) in enumerate(cols):
        inner = [E("c", 20, 20, c or "none", x=4, y=4, stroke="#C3C9D4", strokeWeight=1,
                   **({} if c else {"dashes": [3, 2]}))]
        if i == active:
            inner.append(E("ring", 28, 28, "none", x=0, y=0, stroke=C["primary"], strokeWeight=2))
        kids.append(F("Szín/" + n, inner, w=28, h=28))
    return row("Címke színe", kids, gap=6)


def srow(title, desc, control, last=False):
    t = [T(title, 13, C["text"], "SemiBold")] + ([T(desc, 11, C["muted"], lineHeight="140%")] if desc else [])
    p = dict(gap=12, sizingH="fill", padding=[11, 0, 11, 0])
    if not last:
        p.update(strokeSides=[0, 0, 1, 0], stroke=C["border_s"], strokeBottomWeight=1)
    return row("Beállítás/" + title, [col("t", t, gap=3, sizingH="fill"), control], **p)


def card(title, kids, ic=None, **p):
    head = row("h", ([icon(ic, 16, C["primary"])] if ic else []) + [T(title, 14, C["text"], "Bold")], gap=8)
    p.setdefault("sizingH", "fill")
    return col("Kártya/" + title, [head] + kids, gap=4, padding=[16, 18, 8, 18], cornerRadius=14, fill=C["surface"],
               stroke=C["border"], strokeWeight=1, **p)


def page_header(title, sub, right=None):
    return row("Fejléc", [col("t", [T(title, 24, C["text"], "Bold", letterSpacing="-1%"),
                                    T(sub, 13, C["muted"])], gap=4, sizingH="fill")] + (right or []),
               gap=10, crossAlign="max", sizingH="fill")


def home_frame(name, active, kids, gap=20):
    main = col("Tartalom", kids, gap=gap, padding=[28, 32, 24, 32], sizingH="fill", sizingV="fill")
    body = row("Törzs", [g.home_sidebar(active), main], gap=0, crossAlign="min", sizingH="fill", sizingV="fill")
    return col(name, [g.titlebar(TABS, 0), body], gap=0, w=W, h=H, fill=C["bg"], clip=True, cornerRadius=12)


def modal(frame, title, body, foot, dw, dh):
    head = row("Fej", [T(title, 17, C["text"], "Bold", sizingH="fill"), ibtn("close", size=30)],
               gap=8, h=60, padding=[0, 16, 0, 24], sizingH="fill", strokeSides=[0, 0, 1, 0], stroke=C["border"],
               strokeBottomWeight=1)
    footer = row("Lábléc", foot, gap=10, h=68, padding=[0, 24, 0, 24], sizingH="fill", fill="#FAFBFC",
                 strokeSides=[1, 0, 0, 0], stroke=C["border"], strokeTopWeight=1)
    dialog = col("Ablak/" + title, [head, body, footer], gap=0, w=dw, h=dh, x=(W - dw) / 2, y=(H - dh) / 2,
                 fill=C["surface"], cornerRadius=16, clip=True, effects=g.SHADOW_LG, absolute=True)
    frame["children"] += [R("Háttérsötétítés", W, H, "#0F1424", x=0, y=0, opacity=0.42, absolute=True), dialog]
    return frame


def flex():
    return F("flex", h=1, sizingH="fill")


# ---------- editor panels ----------
def left_panel(title, kids, action="more"):
    return col("Bal panel", [row("Panelfej", [T(title, 15, C["text"], "Bold", sizingH="fill"), ibtn(action, size=28)],
                                 gap=6, h=52, padding=[0, 12, 0, 16], sizingH="fill")] + kids,
               gap=0, w=268, fill=C["surface"], sizingV="fill", strokeSides=[0, 1, 0, 0], stroke=C["border"],
               strokeRightWeight=1)


def right_panel(tab, kids):
    return col("Jobb panel", [row("Fül", [segmented(["Elem", "Címke"], tab)], gap=0, padding=[12, 16, 4, 16],
                                  sizingH="fill")] + kids,
               gap=0, w=300, fill=C["surface"], sizingV="fill", strokeSides=[0, 0, 0, 1], stroke=C["border"],
               strokeLeftWeight=1)


def sel_header(ic, name, sub):
    return row("Kijelölés", [row("kind", [icon(ic, 16, C["primary"])], gap=0, w=32, h=32, mainAlign="center",
                                 cornerRadius=8, fill=C["primary_t"]),
                             col("t", [T(name, 14, C["text"], "Bold"), T(sub, 11, C["muted"])], gap=2, sizingH="fill"),
                             ibtn("more", size=28)], gap=10, padding=[10, 16, 10, 16], sizingH="fill")


def pos_section(x, y, w, h):
    return section("Helyzet és méret", [
        row("r1", [field("X", x, suffix="mm"), field("Y", y, suffix="mm")], gap=8, sizingH="fill"),
        row("r2", [field("SZ", w, suffix="mm"), field("M", h, suffix="mm")], gap=8, sizingH="fill"),
        row("r3", [field(None, "0°", ic="rotate"), row("flip", [ibtn("flip", size=32), ibtn("lock", size=32)], gap=4)],
            gap=8, sizingH="fill")])


def layers_panel():
    items = [(0, "group", "Ár blokk", "group"), (1, "text", "Ár", ""), (1, "text", "Ft/kg", ""),
             (0, "text", "Termék", "sel"), (0, "text", "Leírás", ""), (0, "line", "Elválasztó", ""),
             (0, "barcode", "Vonalkód", ""), (0, "text", "EAN", "hidden"), (0, "date", "Dátum", "locked")]
    rows = []
    for ind, ic, name, st in items:
        on, hid = st == "sel", st == "hidden"
        kids = []
        if st == "group":
            kids.append(icon("chev_down", 12, C["muted"]))
        kids += [icon(ic, 15, C["primary"] if on else C["muted"]),
                 T(name, 12, C["primary_d"] if on else (C["muted"] if hid else C["text"]),
                   "SemiBold" if on or st == "group" else "Regular", name="Label", sizingH="fill")]
        if st == "locked":
            kids.append(icon("warn", 14, C["amber"]))
        kids.append(icon("lock" if st == "locked" else "unlock", 14, C["text2"] if st == "locked" else "#C3C9D4"))
        kids.append(icon("eye_off" if hid else "eye", 14, C["text2"] if hid else "#C3C9D4"))
        rows.append(row("Layer/" + name, kids, gap=8, h=32, padding=[0, 8, 0, 8 + ind * 22 + (0 if st == "group" else 0)],
                        cornerRadius=6, fill=C["primary_t"] if on else "none", sizingH="fill"))
    tools = row("Műveletek", [ibtn("to_front"), ibtn("up"), ibtn("down"), ibtn("to_back"), flex(),
                              ibtn("group"), ibtn("copy"), ibtn("trash", color=C["red"])],
                gap=2, h=48, padding=[0, 10, 0, 10], sizingH="fill", strokeSides=[1, 0, 0, 0], stroke=C["border"],
                strokeTopWeight=1)
    return left_panel("Rétegek", [
        row("s", [field(None, "Réteg keresése…", ic="search", h=32)], gap=0, padding=[0, 16, 10, 16], sizingH="fill"),
        col("Lista", rows, gap=2, padding=[4, 8, 8, 8], sizingH="fill"),
        F("flex", w=1, sizingV="fill"),
        row("tipp", [T("Húzd az átrendezéshez · dupla katt: átnevezés", 11, C["muted"])], gap=0,
            padding=[0, 16, 10, 16], sizingH="fill"),
        tools], action="plus")


def label_settings_panel():
    def finding(kind, title, sub):
        col_ = C["amber"] if kind == "warn" else C["green"]
        return row("Találat/" + title, [icon("warn" if kind == "warn" else "check", 15, col_),
                                        col("t", [T(title, 12, C["text"], "SemiBold"), T(sub, 11, C["muted"],
                                                                                      lineHeight="140%")],
                                            gap=2, sizingH="fill")],
                   gap=8, crossAlign="min", padding=10, cornerRadius=8,
                   fill=C["amber_t"] if kind == "warn" else C["green_t"], sizingH="fill")
    return right_panel(1, [
        section("Címke", [
            row("r", [field("SZ", "50", suffix="mm"), field("M", "30", suffix="mm")], gap=8, sizingH="fill"),
            row("r2", [segmented(["Fekvő", "Álló"], 0), ibtn("rotate", size=30)], gap=8, sizingH="fill"),
        ]),
        section("Papír", [
            field(None, "Résközös címke", ic="chev_down"),
            row("c", [swatches(0), flex(), T("Fehér", 12, C["text2"])], gap=6, sizingH="fill"),
        ]),
        section("Nyomtató", [
            field(None, "NIIMBOT B21 Pro", ic="chev_down"),
            row("d", [T("Sötétség", 12, C["text2"], sizingH="fill"), T("3 / 5", 12, C["text"], "SemiBold")], gap=6,
                sizingH="fill"),
            slider(268, 0.5),
        ]),
        section("Rács és illesztés", [
            row("g", [T("Rács megjelenítése", 12, C["text2"], sizingH="fill"), field(None, "5", suffix="mm", w=76, h=28),
                      toggle(True)], gap=8, sizingH="fill"),
            row("s", [T("Illesztés segédvonalakhoz", 12, C["text2"], sizingH="fill"), toggle(True)], gap=8,
                sizingH="fill"),
        ]),
        section("Ellenőrzés", [
            finding("warn", "A „Dátum” kilóg a nyomtatható területről", "Bal szél: 0,4 mm-rel a margón kívül."),
            finding("ok", "Vonalkód olvasható", "EAN-13 · modulszélesség 0,33 mm"),
        ], action=T("Újra", 11, C["primary"], "SemiBold")),
    ])


def grid_and_warning(lx, ly, s):
    out = [R("rács", 1, 30 * s, "#4C6EF526", x=lx + i * 5 * s, y=ly) for i in range(1, 10)]
    out += [R("rács", 50 * s, 1, "#4C6EF526", x=lx, y=ly + i * 5 * s) for i in range(1, 6)]
    out.append(R("Figyelmeztetés", 13.6 * s, 3.6 * s, "#F59F0014", x=lx + 0.8 * s, y=ly + 25 * s, stroke=C["amber"],
                 strokeWeight=1.5, dashes=[4, 3], cornerRadius=3))
    out.append(dict(row("jelvény", [icon("warn", 12, "#FFFFFF", 2), T("Margón kívül", 10, "#FFFFFF", "SemiBold")],
                        gap=4, h=20, padding=[0, 8, 0, 6], cornerRadius=10, fill=C["amber"]),
                    x=lx + 0.8 * s, y=ly + 29.2 * s))
    return out


def templates_panel():
    items = [("Árcédula", "50 × 30"), ("Névkártya", "70 × 40"), ("Leltárcímke", "50 × 30"), ("QR-link", "50 × 30"),
             ("Befőttesüveg", "50 × 50"), ("Kábelzászló", "14 × 30"), ("Wi-Fi", "50 × 30"), ("Címzés", "70 × 40")]

    def tc(k, sz, on):
        return col("Sablon/" + k, [g.template_thumb(k, w=99, h=66),
                                   col("m", [T(k, 12, C["text"], "SemiBold"), T(sz + " mm", 10, C["muted"])], gap=1,
                                       padding=[0, 2, 0, 2], sizingH="fill")],
                   gap=6, padding=6, cornerRadius=10, stroke=C["primary"] if on else C["border"],
                   strokeWeight=2 if on else 1, fill=C["surface"], sizingH="fill")
    rows = [row("r", [tc(*items[i], i == 0), tc(*items[i + 1], False)], gap=8, sizingH="fill")
            for i in range(0, 8, 2)]
    return left_panel("Sablonok", [
        col("f", [field(None, "Keresés", ic="search", h=32), segmented(["Kezdő", "Saját"], 0)], gap=8,
            padding=[0, 16, 12, 16], sizingH="fill"),
        col("Lista", rows, gap=8, padding=[0, 16, 12, 16], sizingH="fill"),
        F("flex", w=1, sizingV="fill"),
        row("save", [button("Mentés sablonként…", "secondary", ic="plus", h=34, name="Mentés sablonként")], gap=0,
            padding=[10, 16, 12, 16], mainAlign="center", sizingH="fill", strokeSides=[1, 0, 0, 0], stroke=C["border"],
            strokeTopWeight=1)], action="folder")


def barcode_panel():
    return right_panel(0, [
        sel_header("barcode", "Vonalkód", "Vonalkód · kötve: EAN oszlop"),
        pos_section("28,4", "15,8", "19,2", "11,2"),
        section("Vonalkód", [
            field("Típus", "EAN-13", ic="chev_down"),
            field("Érték", "5998001234567"),
            row("ok", [icon("check", 13, C["green"]), T("Érvényes · ellenőrző számjegy: 7", 11, C["green"], "Medium")],
                gap=6),
            row("t", [T("Olvasható szöveg", 12, C["text2"], sizingH="fill"), toggle(True)], gap=8, sizingH="fill"),
            row("p", [T("Szöveg helye", 12, C["text2"], sizingH="fill"), segmented(["Alul", "Felül"], 0, w=130)], gap=8,
                sizingH="fill"),
        ]),
        section("Tartalom", [
            row("Kötés", [icon("link", 15, C["primary"]), T("EAN", 12, C["primary_d"], "SemiBold", sizingH="fill"),
                          T("CSV oszlop", 11, C["muted"]), icon("close", 12, C["muted"])],
                gap=8, h=34, padding=[0, 10, 0, 10], cornerRadius=8, fill=C["primary_t"], sizingH="fill"),
            row("img", [icon("scan", 15, C["text2"]), T("Kód beolvasása képből…", 12, C["text2"], "Medium")], gap=8,
                h=32, padding=[0, 10, 0, 10], cornerRadius=8, stroke=C["border"], strokeWeight=1, sizingH="fill"),
        ]),
        g.arrange_section(),
    ])


# ---------- home-shell pages ----------
def template_library():
    def frow(ic, name, count, on=False, indent=0):
        return row("Mappa/" + name, [icon(ic, 16, C["primary"] if on else C["text2"]),
                                     T(name, 13, C["primary_d"] if on else C["text"], "SemiBold" if on else "Medium",
                                       sizingH="fill"), T(count, 11, C["muted"], "Medium")],
                   gap=10, h=36, padding=[0, 10, 0, 10 + indent], cornerRadius=8,
                   fill=C["primary_t"] if on else "none", sizingH="fill")
    folders = col("Mappák", [
        frow("sparkle", "Kezdő sablonok", "10"),
        F("sp", h=6, w=1),
        T("SAJÁT SABLONOK", 11, C["muted"], "SemiBold", letterSpacing="4%"),
        frow("folder", "Alapértelmezett", "6"), frow("folder", "Bolt", "12", on=True),
        frow("folder", "Iroda", "4"), frow("folder", "Otthon", "7"),
        row("új", [icon("plus", 15, C["primary"]), T("Új mappa", 13, C["primary"], "SemiBold")], gap=10, h=36,
            padding=[0, 10, 0, 10], sizingH="fill"),
    ], gap=4, w=210, padding=12, cornerRadius=14, fill=C["surface"], stroke=C["border"], strokeWeight=1,
        sizingV="fill")
    data = [("Árcédula", "Akciós ár", "50 × 30"), ("Árcédula", "Polccímke", "50 × 30"),
            ("Leltárcímke", "Raktári kód", "50 × 30"), ("QR-link", "Étlap QR", "50 × 30"),
            ("Címzés", "Csomagcímke", "70 × 40"), ("Névkártya", "Eladó kitűző", "70 × 40"),
            ("Befőttesüveg", "Házi lekvár", "50 × 50"), ("Wi-Fi", "Vendég Wi-Fi", "50 × 30"),
            ("Kábelzászló", "Kasszakábel", "14 × 30")]
    cards = [g.tcard(k, t, s, selected=(i == 0), tw=172, th=100) for i, (k, t, s) in enumerate(data)]
    grid = col("Rács", [
        row("h", [T("Bolt", 15, C["text"], "Bold"), T("12 sablon", 12, C["muted"]), flex(),
                  ibtn("pencil", size=28), ibtn("trash", size=28), segmented(["@templates", "@t_left"], 0, w=72)],
            gap=8, sizingH="fill"),
        row("r1", cards[:3], gap=14, sizingH="fill"), row("r2", cards[3:6], gap=14, sizingH="fill"),
        row("r3", cards[6:], gap=14, sizingH="fill")], gap=14, sizingH="fill")

    def meta(k, v):
        return row("m/" + k, [T(k, 12, C["muted"], sizingH="fill"), T(v, 12, C["text"], "Medium")], gap=8,
                   sizingH="fill")

    def act(ic, t, color=None):
        return row("Művelet/" + t, [icon(ic, 15, color or C["text2"]), T(t, 13, color or C["text"], "Medium")], gap=10,
                   h=34, padding=[0, 8, 0, 8], cornerRadius=8, sizingH="fill")
    detail = col("Részletek", [
        g.template_thumb("Árcédula", w=252, h=160),
        col("t", [T("Akciós ár", 17, C["text"], "Bold"), T("50 × 30 mm · Résközös címke", 12, C["muted"])], gap=3,
            sizingH="fill"),
        col("meta", [meta("Mappa", "Bolt"), meta("Módosítva", "2026. nov. 12."), meta("Elemek", "6"),
                     meta("Adatmezők", "Termék, Ár, EAN")], gap=8, padding=[12, 0, 12, 0], sizingH="fill",
            strokeSides=[1, 0, 1, 0], stroke=C["border_s"], strokeTopWeight=1, strokeBottomWeight=1),
        button("Megnyitás", "primary", ic="open", h=38, name="Megnyitás"),
        col("acts", [act("copy", "Másolat"), act("pencil", "Átnevezés"), act("folder", "Áthelyezés…"),
                     act("trash", "Törlés", C["red"])], gap=2, sizingH="fill"),
    ], gap=14, w=284, padding=16, cornerRadius=14, fill=C["surface"], stroke=C["border"], strokeWeight=1,
        sizingV="fill")
    detail["children"][3]["props"]["sizingH"] = "fill"
    return home_frame("7 · Sablonkönyvtár", "Sablonok", [
        page_header("Sablonok", "Kezdő sablonok és saját mappák – dupla kattintással megnyílik.",
                    [field(None, "Keresés a sablonok között…", w=300, ic="search", h=38),
                     button("Mentés sablonként…", "secondary", ic="plus", h=38)]),
        row("Alsó", [folders, grid, detail], gap=20, crossAlign="min", sizingH="fill", sizingV="fill")])


def printers_page():
    def dev(name, sub, state, on=False):
        dot = {"on": C["green"], "off": "#C3C9D4", "new": C["primary"]}[state]
        right = (chip("Csatlakozva", color=C["green"], bg=C["green_t"]) if state == "on"
                 else button("Csatlakozás", "secondary", h=30, name="Csatlakozás"))
        return row("Eszköz/" + name, [
            row("i", [icon("printer", 20, C["primary"] if on else C["text2"])], gap=0, w=44, h=44, mainAlign="center",
                cornerRadius=10, fill=C["primary_t"] if on else "#F1F3F7"),
            col("t", [T(name, 14, C["text"], "SemiBold"), row("s", [g.status_dot(dot), T(sub, 11, C["muted"])], gap=6)],
                gap=3, sizingH="fill"), right],
            gap=12, padding=12, cornerRadius=12, fill=C["surface"], stroke=C["primary"] if on else C["border"],
            strokeWeight=2 if on else 1, sizingH="fill")
    left = col("Lista", [
        T("ESZKÖZEIM", 11, C["muted"], "SemiBold", letterSpacing="4%"),
        dev("NIIMBOT B21 Pro", "USB · 82% · 50 × 30 mm papír", "on", on=True),
        dev("NIIMBOT D110", "Bluetooth · utoljára: nov. 9.", "off"),
        F("sp", h=6, w=1),
        row("h", [T("A KÖZELBEN", 11, C["muted"], "SemiBold", letterSpacing="4%", sizingH="fill"),
                  icon("rotate", 13, C["primary"]), T("Keresés…", 11, C["primary"], "SemiBold")], gap=6, sizingH="fill"),
        dev("NIIMBOT B1", "Bluetooth · új eszköz", "new"),
        row("kézi", [icon("plus", 15, C["text2"]), T("Cím megadása kézzel…", 13, C["text2"], "Medium")], gap=10, h=44,
            padding=[0, 14, 0, 14], cornerRadius=12, stroke=C["border"], strokeWeight=1, dashes=[4, 3], sizingH="fill"),
        F("flex", w=1, sizingV="fill"),
        row("tipp", [icon("info", 15, C["muted"]),
                     T("Nem találod? Kapcsold be a nyomtatót, és USB-n csatlakoztasd, vagy engedélyezd a Bluetooth-t.",
                       11, C["muted"], lineHeight="150%", sizingH="fill")], gap=8, crossAlign="min", sizingH="fill"),
    ], gap=10, w=420, sizingV="fill")

    def stat(k, v, sub, ic, bar=None):
        kids = [row("h", [icon(ic, 14, C["muted"]), T(k, 11, C["muted"], "Medium")], gap=6),
                T(v, 18, C["text"], "Bold"), T(sub, 11, C["muted"])]
        if bar is not None:
            kids.append(F("bar", [R("bg", 120, 5, "#E4E7EC", x=0, y=0, cornerRadius=3),
                                  R("v", 120 * bar, 5, C["green"], x=0, y=0, cornerRadius=3)], w=120, h=5))
        return col("Adat/" + k, kids, gap=6, padding=14, cornerRadius=12, fill="#F7F8FA", sizingH="fill", sizingV="fill")
    illo = F("illusztráció", [
        R("ház", 96, 70, "#FFFFFF", x=12, y=26, cornerRadius=14, stroke=C["border"], strokeWeight=1.5),
        R("nyílás", 56, 5, "#1B2130", x=32, y=40, cornerRadius=3),
        R("címke", 44, 30, "#FFFFFF", x=38, y=10, cornerRadius=3, stroke="#C3C9D4", strokeWeight=1),
        R("sor1", 26, 3, "#1B2130", x=44, y=17, cornerRadius=1), R("sor2", 18, 3, "#8A93A6", x=44, y=24, cornerRadius=1),
        E("led", 6, 6, C["green"], x=22, y=82)], w=120, h=104)
    head = row("fej", [illo, col("t", [
        row("n", [T("NIIMBOT B21 Pro", 20, C["text"], "Bold"),
                  chip("Csatlakozva · USB", color=C["green"], bg=C["green_t"])], gap=10),
        T("Sorozatszám: B21P-48K2-0193 · Firmware 1.24", 12, C["muted"]),
        row("b", [button("Tesztcímke nyomtatása", "secondary", ic="printer", h=32),
                  button("Kalibrálás", "secondary", h=32), button("Leválasztás", "ghost", h=32)], gap=8)],
        gap=10, sizingH="fill")], gap=18, sizingH="fill")
    stats = row("adatok", [stat("Akkumulátor", "82%", "Töltés: nem", "battery", 0.82),
                           stat("Papír", "50 × 30 mm", "Fehér · résközös", "rect"),
                           stat("Felbontás", "203 dpi", "Nyomtatható: 48 mm", "ruler" if "ruler" in g.ICONS else "grid"),
                           stat("Nyomtatva", "1 284", "címke összesen", "clock")], gap=12, crossAlign="min", h=104, sizingH="fill")
    settings = col("beáll", [
        srow("Alapértelmezett sötétség", "A címke saját beállítása felülírja.", row("s", [slider(180, 0.5),
                                                                                         T("3", 13, C["text"], "SemiBold")],
                                                                                    gap=12)),
        srow("Papírtípus", "Résközös, fekete jelöléses vagy folyamatos.", field(None, "Résközös címke", w=200,
                                                                              ic="chev_down")),
        srow("Automatikus kikapcsolás", None, field(None, "15 perc után", w=200, ic="chev_down")),
        srow("Ügynökök nyomtathatnak", "MCP-kliensek csak bekapcsolva küldhetnek nyomtatást erre az eszközre.",
             toggle(False), last=True)], gap=0, sizingH="fill")
    detail = col("Részletek", [head, stats, settings], gap=20, padding=22, cornerRadius=16, fill=C["surface"],
                 stroke=C["border"], strokeWeight=1, sizingH="fill", sizingV="fill")
    return home_frame("8 · Nyomtatók", "Nyomtatók", [
        page_header("Nyomtatók", "USB-n és Bluetooth-on csatlakozó címkenyomtatók.",
                    [button("Nyomtatók keresése", "primary", ic="search", h=38)]),
        row("Alsó", [left, detail], gap=24, crossAlign="min", sizingH="fill", sizingV="fill")])


def history_page():
    widths = [330, 170, 180, 90, 150]
    hdr = ["Címke", "Időpont", "Nyomtató", "Darab", "Állapot"]
    data = [("Árcédula", "50 × 30 mm · termekek.csv", "ma, 14:02", "B21 Pro", "24", "ok"),
            ("Kábelzászló", "14 × 30 mm", "ma, 11:40", "B21 Pro", "8", "ok"),
            ("Fűszerek", "40 × 30 mm · fuszerek.xlsx", "tegnap, 18:21", "D110", "12", "ok"),
            ("Leltár 2026", "50 × 30 mm · sorszám 0001–0050", "nov. 12., 09:15", "B21 Pro", "32 / 50", "stop"),
            ("Költözős dobozok", "70 × 40 mm", "nov. 9., 16:48", "B21 Pro", "16", "ok"),
            ("Wi-Fi vendég", "50 × 30 mm", "nov. 7., 10:02", "D110", "2", "ok"),
            ("Csomagcímke", "70 × 40 mm · rendelesek.csv", "nov. 5., 13:30", "B21 Pro", "0 / 9", "err")]

    def th():
        return row("fej", [T(h_, 11, C["muted"], "SemiBold", width=widths[i], textAutoResize="height")
                           for i, h_ in enumerate(hdr)], gap=0, h=36, padding=[0, 16, 0, 16], fill="#F7F8FA",
                   sizingH="fill", strokeSides=[0, 0, 1, 0], stroke=C["border_s"], strokeBottomWeight=1)

    def tr(n, sub, when, pr, cnt, st):
        stc = {"ok": ("Kész", C["green"], C["green_t"], "check"), "stop": ("Megszakítva", C["amber"], C["amber_t"], "pause"),
               "err": ("Papír kifogyott", C["red"], "#FFF0F0", "warn")}[st]
        return row("Sor/" + n, [
            row("c0", [row("i", [icon("file", 15, C["primary"])], gap=0, w=32, h=32, mainAlign="center", cornerRadius=8,
                           fill=C["primary_t"]),
                       col("t", [T(n, 13, C["text"], "SemiBold"), T(sub, 11, C["muted"])], gap=2, sizingH="fill")],
                gap=10, w=widths[0]),
            T(when, 12, C["text2"], width=widths[1], textAutoResize="height"),
            T(pr, 12, C["text2"], width=widths[2], textAutoResize="height"),
            T(cnt, 12, C["text"], "SemiBold", width=widths[3], textAutoResize="height"),
            row("st", [chip(stc[0], ic=stc[3], color=stc[1], bg=stc[2])], gap=0, w=widths[4]),
            flex(), button("Újranyomtatás" if st == "ok" else "Folytatás", "secondary", ic="printer", h=30,
                           name="Újranyomtatás"), ibtn("more", size=30)],
            gap=0, h=54, padding=[0, 12, 0, 16], sizingH="fill", strokeSides=[0, 0, 1, 0], stroke=C["border_s"],
            strokeBottomWeight=1)
    job = row("Aktív feladat", [
        row("i", [icon("printer", 20, "#FFFFFF")], gap=0, w=44, h=44, mainAlign="center", cornerRadius=10,
            fill=C["primary"]),
        col("t", [row("h", [T("Nyomtatás folyamatban", 14, C["text"], "Bold"),
                            T("Polccímke · B21 Pro", 12, C["text2"]), flex(),
                            T("14 / 24 címke", 12, C["text"], "SemiBold")], gap=10, sizingH="fill"),
                  F("bar", [R("bg", 840, 6, "#DBE4FF", x=0, y=0, cornerRadius=3),
                            R("v", 490, 6, C["primary"], x=0, y=0, cornerRadius=3)], w=840, h=6)], gap=10,
            sizingH="fill"),
        button("Szünet", "secondary", ic="pause", h=34), button("Megszakítás", "ghost", h=34)],
        gap=14, padding=[14, 16, 14, 16], cornerRadius=14, fill=C["primary_t"], stroke=C["primary_t2"], strokeWeight=1,
        sizingH="fill")
    table = col("Táblázat", [th()] + [tr(*d) for d in data], gap=0, cornerRadius=14, fill=C["surface"],
                stroke=C["border"], strokeWeight=1, clip=True, sizingH="fill")
    return home_frame("9 · Előzmények", "Előzmények", [
        page_header("Előzmények", "Minden nyomtatás a gépeden marad – bármelyik újranyomtatható.",
                    [field(None, "Keresés…", w=240, ic="search", h=38), button("Előzmények törlése", "secondary", h=38)]),
        job,
        row("szűrők", [chip("Mind", active=True), chip("Ma"), chip("7 nap"), chip("30 nap"), g.divider_v(16),
                       chip("B21 Pro", ic="printer"), chip("D110", ic="printer"), flex(),
                       T("7 nyomtatás · 94 címke", 12, C["muted"])], gap=8, sizingH="fill"),
        table], gap=18)


def settings_page():
    def nav(ic, t, on=False):
        return row("Nav/" + t, [icon(ic, 16, C["primary"] if on else C["text2"]),
                                T(t, 13, C["primary_d"] if on else C["text"], "SemiBold" if on else "Medium")],
                   gap=10, h=36, padding=[0, 10, 0, 10], cornerRadius=8, fill=C["primary_t"] if on else "none",
                   sizingH="fill")
    subnav = col("Kategóriák", [nav("settings", "Általános", True), nav("sun", "Megjelenés"), nav("pencil", "Szerkesztő"),
                                nav("printer", "Nyomtatás"), nav("agent", "Ügynökök (MCP)"), nav("info", "Névjegy")],
                 gap=2, w=200, padding=10, cornerRadius=14, fill=C["surface"], stroke=C["border"], strokeWeight=1)
    canvas_sw = row("vászon", [F("s/" + c, [R("c", 22, 22, c, x=3, y=3, cornerRadius=6, stroke="#C3C9D4",
                                               strokeWeight=1)] +
                                 ([R("ring", 28, 28, "none", x=0, y=0, cornerRadius=9, stroke=C["primary"],
                                     strokeWeight=2)] if i == 0 else []), w=28, h=28)
                               for i, c in enumerate(["#E8EBF0", "#FFFFFF", "#F4EFE6", "#2B3040"])], gap=6)
    c1 = col("bal", [
        card("Általános", [
            srow("Nyelv", "A menük és panelek nyelve.", field(None, "Magyar", w=170, ic="chev_down")),
            srow("Mértékegység", None, segmented(["mm", "hüvelyk"], 0, w=170)),
            srow("Indításkor", None, segmented(["Kezdőlap", "Legutóbbi"], 0, w=170)),
            srow("Automatikus mentés", "A megnyitott címkék 30 másodpercenként mentődnek.", toggle(True), last=True),
        ], ic="settings"),
        card("Megjelenés", [
            srow("Téma", None, segmented(["Világos", "Sötét", "Rendszer"], 0, w=220)),
            srow("Vászon háttérszíne", None, canvas_sw),
            srow("Vonalzók megjelenítése", None, toggle(True), last=True),
        ], ic="sun"),
        card("Szerkesztő", [
            srow("Rács", "Rácsköz milliméterben.", row("r", [field(None, "1", suffix="mm", w=80), toggle(False)], gap=10)),
            srow("Illesztés segédvonalakhoz", None, toggle(True)),
            srow("Alapértelmezett betűtípus", None, field(None, "Inter", w=170, ic="chev_down"), last=True),
        ], ic="pencil"),
    ], gap=16, sizingH="fill")

    def code(text, ic="copy"):
        return row("kód", [T(text, 12, C["text"], "Medium", sizingH="fill"), icon(ic, 14, C["text2"])], gap=10, h=32,
                   w=230, padding=[0, 10, 0, 10], cornerRadius=7, fill="#F1F3F7")
    c2 = col("jobb", [
        card("Ügynökök (MCP)", [
            srow("MCP-szerver", "AI-ügynökök ugyanazokat a parancsokat használhatják, mint te.", toggle(True)),
            srow("Cím", "Csak erről a gépről érhető el.", code("127.0.0.1:7878/mcp")),
            srow("Hozzáférési token", "Az mcp-connection.json fájlban is megtalálod.", code("••••••••••••3f9a")),
            srow("Ügynökök nyomtathatnak", "Kikapcsolva minden nyomtatás a te jóváhagyásodra vár.", toggle(False)),
            srow("Engedélyezett mappa", "Az ügynök csak itt olvashat és írhat fájlokat.", code("~/Címkék", "folder"),
                 last=True),
        ], ic="agent"),
        card("Nyomtatás", [
            srow("Ellenőrzés nyomtatás előtt", "Figyelmeztet, ha egy elem kilóg vagy a kód nem olvasható.", toggle(True)),
            srow("Alapértelmezett példányszám", None, field(None, "1", w=80), last=True),
        ], ic="printer"),
        card("Névjegy", [
            row("n", [col("t", [T("Labely 0.3.0", 13, C["text"], "SemiBold"),
                                T("Szabad és nyílt forráskódú (MIT VAGY Apache-2.0). Internet nélkül is működik, "
                                  "fiók és felhő nélkül.", 11, C["muted"], lineHeight="150%")], gap=3, sizingH="fill"),
                      button("Dokumentáció", "secondary", h=30)], gap=12, sizingH="fill", padding=[8, 0, 10, 0]),
        ], ic="info"),
    ], gap=16, sizingH="fill")
    return home_frame("10 · Beállítások", "Beállítások", [
        page_header("Beállítások", "Minden beállítás ezen a gépen tárolódik."),
        row("Alsó", [subnav, c1, c2], gap=20, crossAlign="min", sizingH="fill", sizingV="fill")])


# ---------- dialogs ----------
def new_label_dialog():
    sizes = ["50 × 30", "40 × 30", "30 × 15", "50 × 50", "40 × 12", "70 × 40", "14 × 30 kábel", "Egyéni…"]
    form = col("Űrlap", [
        col("g1", [lab("Méret"), row("r1", [chip(s + (" mm" if s[0].isdigit() and "kábel" not in s else ""),
                                              active=(i == 0)) for i, s in enumerate(sizes[:4])], gap=6),
                   row("r2", [chip(s + (" mm" if s[0].isdigit() and "kábel" not in s else ""))
                              for s in sizes[4:]], gap=6)], gap=8, sizingH="fill"),
        row("g2", [col("a", [lab("Szélesség"), field(None, "50", suffix="mm", h=36)], gap=8, sizingH="fill"),
                   col("b", [lab("Magasság"), field(None, "30", suffix="mm", h=36)], gap=8, sizingH="fill"),
                   col("c", [lab("Tájolás"), segmented(["Fekvő", "Álló"], 0, w=150)], gap=8, w=150)],
            gap=14, crossAlign="max", sizingH="fill"),
        row("g3", [col("a", [lab("Papírtípus"), field(None, "Résközös címke", ic="chev_down", h=36)], gap=8,
                       sizingH="fill"),
                   col("b", [lab("Nyomtatótípus"), field(None, "NIIMBOT B21 Pro", ic="chev_down", h=36)], gap=8,
                       sizingH="fill")], gap=14, sizingH="fill"),
        col("g4", [lab("Címke színe"), row("r", [swatches(0), T("Fehér", 12, C["text2"])], gap=10)], gap=8,
            sizingH="fill"),
    ], gap=20, padding=24, sizingH="fill", sizingV="fill")
    s = 4.4
    stage = F("Előnézet", [
        R("címke", 50 * s, 30 * s, "#FFFFFF", x=36, y=40, cornerRadius=2.5 * s, effects=g.SHADOW_MD),
        R("terület", 50 * s - 12, 30 * s - 12, "none", x=42, y=46, cornerRadius=8, stroke="#4C6EF566", strokeWeight=1,
          dashes=[4, 4]),
        R("mérő", 50 * s, 1, C["muted"], x=36, y=24), R("m1", 1, 9, C["muted"], x=36, y=20),
        R("m2", 1, 9, C["muted"], x=36 + 50 * s, y=20),
        T("50 mm", 11, C["text2"], "SemiBold", x=36 + 25 * s - 18, y=4),
        R("mérőv", 1, 30 * s, C["muted"], x=20, y=40), R("m3", 9, 1, C["muted"], x=16, y=40),
        R("m4", 9, 1, C["muted"], x=16, y=40 + 30 * s),
        T("30", 11, C["text2"], "SemiBold", x=2, y=40 + 15 * s - 7),
    ], w=276, h=190)
    side = col("Oldal", [stage,
                         col("tipp", [row("h", [g.status_dot(C["green"]), T("B21 Pro csatlakoztatva", 12, C["text"],
                                                                         "SemiBold")], gap=6),
                                      T("A nyomtatóban 50 × 30 mm-es fehér, résközös papír van.", 11, C["text2"],
                                        lineHeight="150%"),
                                      row("b", [chip("Méret átvétele", active=True, ic="check")], gap=0)],
                             gap=8, padding=12, cornerRadius=10, fill=C["surface"], stroke=C["border"], strokeWeight=1,
                             sizingH="fill")],
               gap=16, w=308, padding=16, fill="#FAFBFC", mainAlign="center", sizingV="fill", strokeSides=[0, 0, 0, 1],
               stroke=C["border"], strokeLeftWeight=1)
    body = row("tartalom", [form, side], gap=0, crossAlign="min", sizingH="fill", sizingV="fill")
    fr = g.home()
    fr["name"] = "11 · Új címke"
    return modal(fr, "Új címke", body, [row("h", [icon("templates", 14, C["primary"]),
                                                  T("Inkább sablonból…", 12, C["primary"], "SemiBold")], gap=6),
                                        flex(), button("Mégse", "secondary"), button("Létrehozás", "primary")], 800, 510)


def export_dialog():
    body = col("tartalom", [
        col("a", [lab("Formátum"), segmented(["PNG", "PDF"], 0)], gap=8, sizingH="fill"),
        col("b", [lab("Felbontás"), segmented(["203 dpi", "300 dpi", "600 dpi"], 1)], gap=8, sizingH="fill"),
        col("c", [lab("Rekordok"), segmented(["Aktuális rekord", "Mind a 24"], 1)], gap=8, sizingH="fill"),
        col("d", [srow("Nyomtatási kép (1 bites)", "Úgy exportál, ahogy a hőnyomtató nyomtatná.", toggle(False)),
                  srow("Átlátszó háttér", None, toggle(True), last=True)], gap=0, sizingH="fill"),
        col("e", [lab("Mentés helye"), field(None, "~/Címkék/Árcédula-{rekord}.png", ic="folder", h=36),
                  T("24 fájl készül: Árcédula-001.png … Árcédula-024.png", 11, C["muted"])], gap=8, sizingH="fill"),
    ], gap=16, padding=[20, 24, 16, 24], sizingH="fill", sizingV="fill")
    fr = g.editor("12 · Exportálás", g.insert_panel())
    return modal(fr, "Exportálás", body, [flex(), button("Mégse", "secondary"),
                                          button("Exportálás", "primary", ic="download")], 540, 588)


# ---------- build ----------
g.GRID_ON = False
frames = [g.home(), g.editor("2 · Szerkesztő", g.insert_panel()), g.print_dialog(),
          g.editor("4 · Adatok és rekordok", g.data_panel(), with_data=True)]
g.GRID_ON = True
frames.append(g.editor("5 · Rétegek és címkebeállítások", layers_panel(), right=label_settings_panel(),
                       rail_active="Rétegek", sel=(2.4, 2.2, 28, 7.4), extra=grid_and_warning,
                       status="9 elem · 1 kijelölve · 1 figyelmeztetés · Mentve 2 perce"))
g.GRID_ON = False
frames.append(g.editor("6 · Sablonok és vonalkód", templates_panel(), right=barcode_panel(), rail_active="Sablonok",
                       sel=(28.4, 15.8, 19.2, 11.2)))
frames += [template_library(), printers_page(), history_page(), settings_page(), new_label_dialog(), export_dialog()]
for i, f in enumerate(frames):
    f["x"] = (i % 4) * 1560
    f["y"] = (i // 4) * 1020

spec = {
    "styles": {"paint": {"Labely/Primary": C["primary"], "Labely/Primary tint": C["primary_t"],
                         "Labely/Text": C["text"], "Labely/Muted": C["muted"], "Labely/Border": C["border"],
                         "Labely/Canvas": C["canvas"], "Labely/Background": C["bg"], "Labely/Success": C["green"],
                         "Labely/Warning": C["amber"], "Labely/Danger": C["red"]},
               "text": {"Heading/XL": {"fontFamily": "Inter", "fontStyle": "Bold", "fontSize": 26},
                        "Heading/L": {"fontFamily": "Inter", "fontStyle": "Bold", "fontSize": 17},
                        "Heading/M": {"fontFamily": "Inter", "fontStyle": "Bold", "fontSize": 15},
                        "Body": {"fontFamily": "Inter", "fontStyle": "Regular", "fontSize": 13},
                        "Caption": {"fontFamily": "Inter", "fontStyle": "Medium", "fontSize": 11}}},
    "frames": frames,
    "save": "labely-desktop.figcraft",
    "exports": [{"frames": "each", "dir": "png", "format": "png", "scale": 1}],
    "preview": "preview.png",
}
json.dump(spec, open(sys.argv[1], "w"), ensure_ascii=False, indent=1)
print("ok", len(frames))
