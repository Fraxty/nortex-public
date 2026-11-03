#!/usr/bin/env python3
"""Erzeugt die PDFs: Benutzerhandbuch (Kapitel 01..13) und Sprachreferenz (Kapitel 01..15), je de/en.

Nur Python-Standardbibliothek. Aufruf (aus dem Repo-Root):
    python scripts/build_manual_pdf.py
"""
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
MANUAL = ROOT / "docs" / "user-manual"
LANGS = {
    "de": dict(src=MANUAL, out=MANUAL / "NORTEX-Code-Handbuch.pdf", title="Benutzerhandbuch",
               toc="Inhaltsverzeichnis", footer="NORTEX Code – Handbuch",
               meta="NORTEX Code - Handbuch", pages="Seiten", chapters="Kapitel"),
    "en": dict(src=MANUAL / "en", out=MANUAL / "NORTEX-Code-Manual.pdf", title="User Manual",
               toc="Table of contents", footer="NORTEX Code – Manual",
               meta="NORTEX Code - Manual", pages="pages", chapters="chapters"),
    "ref-de": dict(src=ROOT / "docs" / "language", out=ROOT / "docs" / "language" / "NORTEX-Sprachreferenz.pdf",
                   title="Sprachreferenz", toc="Inhaltsverzeichnis", footer="NORTEX - Sprachreferenz",
                   meta="NORTEX - Sprachreferenz", pages="Seiten", chapters="Kapitel", last=15),
    "ref-en": dict(src=ROOT / "docs" / "language" / "en", out=ROOT / "docs" / "language" / "NORTEX-Language-Reference.pdf",
                   title="Language Reference", toc="Table of contents", footer="NORTEX - Language Reference",
                   meta="NORTEX - Language Reference", pages="pages", chapters="chapters", last=15),
}
CFG = LANGS["de"]
OUT = CFG["out"]

PW, PH = 595, 842
ML, MR, MT, MB = 62, 62, 70, 70
TEXT_W = PW - ML - MR

HELV = {32: 278, 33: 278, 34: 355, 35: 556, 36: 556, 37: 889, 38: 667, 39: 191, 40: 333, 41: 333,
        42: 389, 43: 584, 44: 278, 45: 333, 46: 278, 47: 278, 58: 278, 59: 278, 60: 584, 61: 584,
        62: 584, 63: 556, 64: 1015, 91: 278, 92: 278, 93: 278, 94: 469, 95: 556, 96: 333,
        123: 334, 124: 260, 125: 334, 126: 584}
for c in range(48, 58):
    HELV[c] = 556
for ch, w in zip("ABCDEFGHIJKLMNOPQRSTUVWXYZ",
                 [667, 667, 722, 722, 667, 611, 778, 722, 278, 500, 667, 556, 833,
                  722, 778, 667, 778, 722, 667, 611, 722, 667, 944, 667, 667, 611]):
    HELV[ord(ch)] = w
for ch, w in zip("abcdefghijklmnopqrstuvwxyz",
                 [556, 556, 500, 556, 556, 278, 556, 556, 222, 222, 500, 222, 833,
                  556, 556, 556, 556, 333, 500, 278, 556, 500, 722, 500, 500, 500]):
    HELV[ord(ch)] = w

REPL = {"→": "->", "←": "<-", "↑": "^", "↓": "v", "≥": ">=", "≤": "<=", "≠": "!=", "✓": "x",
        "✔": "x", "×": "x", "−": "-", "\u00a0": " ", "\u202f": " ", "\u2009": " ", "⇒": "=>",
        "─": "-", "│": "|", "┌": "+", "┐": "+", "└": "+", "┘": "+", "├": "+", "┤": "+",
        "…": "…", "\ufe0f": "", "\u200b": ""}


def to_cp1252(text):
    out = []
    for ch in text:
        ch = REPL.get(ch, ch)
        for c in ch:
            try:
                c.encode("cp1252")
                out.append(c)
            except UnicodeEncodeError:
                out.append("?")
    return "".join(out)


def width(text, font, size):
    if font == "F3":
        return len(text) * 600 * size / 1000
    f = 1.07 if font == "F2" else 1.0
    return sum(HELV.get(ord(c), 556) for c in text) * f * size / 1000


def wrap(text, font, size, maxw):
    lines, cur = [], ""
    for word in text.split(" "):
        while width(word, font, size) > maxw:  # überlanges Wort hart umbrechen
            k = len(word)
            while k > 1 and width(word[:k], font, size) > maxw:
                k -= 1
            if cur:
                lines.append(cur)
                cur = ""
            lines.append(word[:k])
            word = word[k:]
        trial = word if not cur else cur + " " + word
        if width(trial, font, size) <= maxw:
            cur = trial
        else:
            lines.append(cur)
            cur = word
    lines.append(cur)
    return lines


def inline(text):
    text = re.sub(r"!\[([^\]]*)\]\([^)]*\)", r"[Abbildung: \1]", text)
    text = re.sub(r"\[([^\]]*)\]\([^)]*\)", r"\1", text)
    text = re.sub(r"\*\*(.+?)\*\*", r"\1", text)
    text = re.sub(r"__(.+?)__", r"\1", text)
    text = re.sub(r"(?<![\w*])\*(?!\s)(.+?)(?<!\s)\*(?![\w*])", r"\1", text)
    text = re.sub(r"`([^`]*)`", r"\1", text)
    text = re.sub(r"<[^>]+>", "", text)
    text = text.replace("\\|", "|")
    return to_cp1252(text)


def parse(md):
    """Liefert Blöcke: (kind, data)."""
    blocks, para = [], []
    lines = md.replace("\r\n", "\n").replace("\t", "    ").split("\n")
    i = 0

    def flush():
        if para:
            blocks.append(("p", " ".join(s.strip() for s in para)))
            para.clear()

    while i < len(lines):
        ln = lines[i]
        s = ln.strip()
        if s.startswith("```"):
            flush()
            i += 1
            code = []
            while i < len(lines) and not lines[i].strip().startswith("```"):
                code.append(lines[i])
                i += 1
            blocks.append(("code", code))
        elif s.startswith("<!--"):
            flush()
            while i < len(lines) and "-->" not in lines[i]:
                i += 1
        elif re.match(r"#{1,3} ", s):
            flush()
            n = len(s) - len(s.lstrip("#"))
            blocks.append(("h%d" % n, s[n:].strip()))
        elif s.startswith("|"):
            flush()
            rows = []
            while i < len(lines) and lines[i].strip().startswith("|"):
                row = lines[i].strip().strip("|")
                cells = [c.strip() for c in re.split(r"(?<!\\)\|", row)]
                if not all(re.fullmatch(r":?-{2,}:?", c) or c == "" for c in cells) or not cells:
                    rows.append(cells)
                i += 1
            blocks.append(("table", rows))
            continue
        elif re.match(r"(\s*)([-*+]|\d+[.)])\s+", ln):
            flush()
            m = re.match(r"(\s*)([-*+]|\d+[.)])\s+(.*)", ln)
            level = len(m.group(1)) // 2
            marker = "\u2022" if m.group(2) in "-*+" else m.group(2)
            blocks.append(("li", (level, marker, m.group(3).strip())))
        elif s.startswith(">"):
            flush()
            blocks.append(("quote", s.lstrip("> ").strip()))
        elif s == "" or re.fullmatch(r"[-*_]{3,}", s):
            flush()
        elif blocks and blocks[-1][0] == "li" and ln.startswith(" ") and not para:
            lvl, mk, t = blocks[-1][1]
            blocks[-1] = ("li", (lvl, mk, t + " " + s))
        else:
            para.append(s)
        i += 1
    flush()
    return blocks


class Doc:
    def __init__(self):
        self.pages = []  # je Seite: Liste von (font, size, x, y, text)
        self.y = 0
        self.cur = None
        self.toc = []  # (level, title, seite)

    def new_page(self):
        self.cur = []
        self.pages.append(self.cur)
        self.y = PH - MT

    def line(self, text, font, size, x=ML, lead=None, before=0):
        lead = lead or size * 1.35
        if self.y - before - lead < MB:
            self.new_page()
            before = 0
        self.y -= before + lead
        self.cur.append((font, size, x, self.y, text))

    def need(self, h):
        if self.y - h < MB:
            self.new_page()


def render_chapter(doc, blocks, first):
    for kind, data in blocks:
        if kind in ("h1", "h2", "h3"):
            title = inline(data)
            lvl = int(kind[1])
            if lvl == 1:
                doc.new_page()
                size, before = 22, 0
            else:
                size, before = (15, 14) if lvl == 2 else (12, 10)
            doc.need(before + size * 1.35 + 60)
            if lvl > 1 and doc.y == PH - MT:
                before = 0
            doc.toc.append((lvl, title, len(doc.pages)))
            for k, t in enumerate(wrap(title, "F2", size, TEXT_W)):
                doc.line(t, "F2", size, before=before if k == 0 else 0)
            doc.y -= 4
        elif kind == "p" or kind == "quote":
            x = ML + (14 if kind == "quote" else 0)
            for t in wrap(inline(data), "F1", 10.5, TEXT_W - (x - ML)):
                doc.line(t, "F1", 10.5, x=x, lead=14.5)
            doc.y -= 5
        elif kind == "li":
            lvl, marker, text = data
            x0 = ML + 8 + lvl * 16
            indent = 18
            ls = wrap(inline(text), "F1", 10.5, TEXT_W - (x0 - ML) - indent)
            doc.need(30)
            doc.line(inline(marker), "F1", 10.5, x=x0, lead=14.5)
            doc.cur.pop()
            doc.cur.append(("F1", 10.5, x0, doc.y, inline(marker)))
            for k, t in enumerate(ls):
                if k:
                    doc.line(t, "F1", 10.5, x=x0 + indent, lead=14.5)
                else:
                    doc.cur.append(("F1", 10.5, x0 + indent, doc.y, t))
            doc.y -= 2
        elif kind == "code":
            size = 8.5
            maxc = int(TEXT_W - 12) * 1000 // int(600 * size)
            doc.y -= 3
            for raw in data:
                raw = to_cp1252(raw.rstrip())
                parts = [raw[k:k + maxc] for k in range(0, max(len(raw), 1), maxc)] or [""]
                for t in parts:
                    doc.line(t, "F3", size, x=ML + 10, lead=11.5)
            doc.y -= 8
        elif kind == "table":
            doc.y -= 3
            for r, cells in enumerate(data):
                text = " | ".join(inline(c) for c in cells)
                font = "F2" if r == 0 else "F1"
                for k, t in enumerate(wrap(text, font, 9.5, TEXT_W - 26)):
                    doc.line(t, font, 9.5, x=ML + 10 + (12 if k else 0), lead=13)
                doc.y -= 2
            doc.y -= 6


def esc(b):
    return b.replace(b"\\", b"\\\\").replace(b"(", b"\\(").replace(b")", b"\\)")


def page_stream(items, number):
    out = [b"BT"]
    for font, size, x, y, text in items:
        out.append(b"/%s %.2f Tf 1 0 0 1 %.2f %.2f Tm (%s) Tj" % (
            font.encode(), size, x, y, esc(text.encode("cp1252"))))
    out.append(b"ET")
    if number:
        label = to_cp1252(CFG["footer"])
        out.append(b"BT /F1 8.5 Tf 1 0 0 1 %d 40 Tm (%s) Tj ET" % (ML, esc(label.encode("cp1252"))))
        num = str(number)
        out.append(b"BT /F1 8.5 Tf 1 0 0 1 %.2f 40 Tm (%s) Tj ET" % (
            PW - MR - width(num, "F1", 8.5), num.encode()))
    return b"\n".join(out)


def write_pdf(pages, path):
    objs = []  # Index = Objektnummer-1

    def add(b):
        objs.append(b)
        return len(objs)

    cat = add(b"")
    pages_id = add(b"")
    fonts = [add(b"<< /Type /Font /Subtype /Type1 /BaseFont /%s /Encoding /WinAnsiEncoding >>" % n)
             for n in (b"Helvetica", b"Helvetica-Bold", b"Courier")]
    kids = []
    for n, items in enumerate(pages):
        data = page_stream(items, n + 1 if n else 0)
        cid = add(b"<< /Length %d >>\nstream\n" % len(data) + data + b"\nendstream")
        pid = add(b"<< /Type /Page /Parent %d 0 R /MediaBox [0 0 %d %d] /Contents %d 0 R "
                  b"/Resources << /Font << /F1 %d 0 R /F2 %d 0 R /F3 %d 0 R >> >> >>"
                  % (pages_id, PW, PH, cid, *fonts))
        kids.append(pid)
    objs[cat - 1] = b"<< /Type /Catalog /Pages %d 0 R >>" % pages_id
    objs[pages_id - 1] = b"<< /Type /Pages /Kids [%s] /Count %d >>" % (
        b" ".join(b"%d 0 R" % k for k in kids), len(kids))
    info = add(b"<< /Title (" + CFG["meta"].encode("ascii") + b") /Producer (build_manual_pdf.py) >>")
    buf = bytearray(b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n")
    offs = []
    for i, o in enumerate(objs):
        offs.append(len(buf))
        buf += b"%d 0 obj\n" % (i + 1) + o + b"\nendobj\n"
    xref = len(buf)
    buf += b"xref\n0 %d\n0000000000 65535 f \n" % (len(objs) + 1)
    for o in offs:
        buf += b"%010d 00000 n \n" % o
    buf += b"trailer\n<< /Size %d /Root %d 0 R /Info %d 0 R >>\nstartxref\n%d\n%%%%EOF\n" % (
        len(objs) + 1, cat, info, xref)
    path.write_bytes(bytes(buf))


def version():
    for f in (ROOT / "ide" / "Cargo.toml", ROOT / "Cargo.toml"):
        txt = f.read_text(encoding="utf-8")
        m = re.search(r'^version\s*=\s*"([^"]+)"', txt, re.M)
        if m:
            return m.group(1)
    sys.exit("Version nicht gefunden")


def main():
    files = sorted(CFG["src"].glob("[0-1][0-9]-*.md"))
    last = CFG.get("last", 13)
    files = [f for f in files if 1 <= int(f.name[:2]) <= last]
    if len(files) != last:
        sys.exit("Erwartet %d Kapitel, gefunden: %d" % (last, len(files)))

    # Pass 1: Seitenzahl des Inhaltsverzeichnisses bestimmen (Body-Layout ist davon unabhängig)
    body = Doc()
    body.pages = []
    for f in files:
        render_chapter(body, parse(f.read_text(encoding="utf-8-sig")), False)
    entries = [e for e in body.toc if e[0] <= 2]
    per_page = (PH - MT - MB - 40) // 15
    toc_pages = max(1, -(-len(entries) // per_page))
    offset = 1 + toc_pages

    doc = Doc()
    doc.new_page()
    doc.y = PH - 280
    doc.line("NORTEX" if CFG.get("last") else "NORTEX Code", "F2", 40, lead=48)
    doc.line(CFG["title"], "F1", 24, lead=32)
    doc.line("Version " + to_cp1252(version()), "F1", 13, lead=30, before=20)
    for k in range(toc_pages):
        doc.new_page()
        if k == 0:
            doc.line(CFG["toc"], "F2", 20, lead=28)
            doc.y -= 12
    # TOC-Zeilen verteilen
    n = 0
    for pi in range(toc_pages):
        doc.cur = doc.pages[1 + pi]
        y = doc.y if pi == 0 else PH - MT
        if pi == 0:
            y = PH - MT - 28 - 12
        for lvl, title, pg in entries[n:n + per_page]:
            x = ML + (0 if lvl == 1 else 16)
            font = "F2" if lvl == 1 else "F1"
            y -= 15 + (3 if lvl == 1 else 0)
            t = title
            while width(t, font, 10) > TEXT_W - 50 - (x - ML) and len(t) > 4:
                t = t[:-2]
            doc.cur.append((font, 10, x, y, t if t == title else t.rstrip() + "..."))
            num = str(pg + offset)
            doc.cur.append(("F1", 10, PW - MR - width(num, "F1", 10), y, num))
        n += per_page

    doc.toc = []
    for f in files:
        render_chapter(doc, parse(f.read_text(encoding="utf-8-sig")), False)
    assert [(a, b, c + offset) for a, b, c in entries] == [t for t in doc.toc if t[0] <= 2]
    write_pdf(doc.pages, OUT)
    print("%s: %d %s, %d %s" % (OUT, len(doc.pages), CFG["pages"], len(files), CFG["chapters"]))


if __name__ == "__main__":
    langs = [sys.argv[2]] if len(sys.argv) > 2 and sys.argv[1] == "--lang" else list(LANGS)
    for lang in langs:
        CFG = LANGS[lang]
        OUT = CFG["out"]
        main()
