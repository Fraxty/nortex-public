#!/usr/bin/env python3
"""Einmalig: format!("Deutscher Text {x}") -> tr_fmt("Deutscher Text {}", &[x.to_string()]).

Aufruf: python scripts/wrap_i18n_fmt.py [--apply]
Ohne --apply werden nur die gefundenen Vorlagen ausgegeben.
"""
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent / "ide" / "src"
SKIP_FILES = {"terminal.rs", "scope.rs", "settings.rs", "toolchain.rs", "search.rs", "i18n.rs"}
SKIP_SUBSTR = ("temp_dir", "CARGO_PKG", "overflow-checks", "write_crash_log", "{section}",
               "{label}:", "stderr:", "tastenkürzel shortcut", "{index:02}", "nortex-",
               "{name}:{}", "{full_key}", "{label}  {shortcut}", "Compiler Log (", "NORTEX Code", "error[", "> {", "\\r\\n")
GERMAN = re.compile(r"[A-Za-zäöüÄÖÜß]{3,}")
CALL = re.compile(r'format!\(\s*"((?:[^"\\]|\\.)*)"')


def find_close(text, start):
    depth = 0
    i = start
    in_str = False
    while i < len(text):
        c = text[i]
        if in_str:
            if c == "\\":
                i += 1
            elif c == '"':
                in_str = False
        elif c == '"':
            in_str = True
        elif c == "(":
            depth += 1
        elif c == ")":
            depth -= 1
            if depth == 0:
                return i
        i += 1
    return -1


def convert(template, tail):
    names = []

    def repl(m):
        names.append(m.group(1))
        return "{}"

    new = re.sub(r"\{([A-Za-z_][A-Za-z0-9_]*)\}", repl, template)
    if "{:" in new or re.search(r"\{[A-Za-z_]+:", new):
        return None
    args = []
    tail = tail.strip().lstrip(",").strip()
    positional = new.count("{}") - len(names)
    explicit = [a.strip() for a in split_args(tail)] if tail else []
    if positional != len(explicit) and explicit:
        return None
    # Reihenfolge: Platzhalter erscheinen in Textreihenfolge; Namen und Positionsargumente mischen nicht.
    if names and explicit:
        return None
    args = explicit or names
    return new, args


def split_args(tail):
    out, depth, cur, in_str = [], 0, "", False
    i = 0
    while i < len(tail):
        c = tail[i]
        if in_str:
            cur += c
            if c == "\\":
                i += 1
                cur += tail[i]
            elif c == '"':
                in_str = False
        elif c == '"':
            in_str = True
            cur += c
        elif c in "([{":
            depth += 1
            cur += c
        elif c in ")]}":
            depth -= 1
            cur += c
        elif c == "," and depth == 0:
            out.append(cur)
            cur = ""
        else:
            cur += c
        i += 1
    if cur.strip():
        out.append(cur)
    return out


def main():
    apply = "--apply" in sys.argv
    templates = []
    for path in sorted(ROOT.rglob("*.rs")):
        if path.name in SKIP_FILES:
            continue
        text = path.read_text(encoding="utf-8")
        cut = text.find("#[cfg(test)]")
        head, tail_part = (text[:cut], text[cut:]) if cut >= 0 else (text, "")
        out, pos, changed = [], 0, False
        for m in CALL.finditer(head):
            if m.start() < pos:
                continue
            open_paren = m.start() + len("format!")
            close = find_close(head, open_paren)
            if close < 0:
                continue
            whole = head[m.start():close + 1]
            if any(s in whole for s in SKIP_SUBSTR) or not GERMAN.search(m.group(1)) or "{" not in m.group(1):
                continue
            rest = head[m.end():close]
            conv = convert(m.group(1), rest)
            if conv is None:
                continue
            template, args = conv
            argtxt = ", ".join(a if a.endswith(".to_string()") else f"{a}.to_string()" for a in args)
            replacement = f'tr_fmt("{template}", &[{argtxt}])'
            templates.append(template)
            out.append(head[pos:m.start()])
            out.append(replacement)
            pos = close + 1
            changed = True
        if changed:
            out.append(head[pos:])
            if apply:
                new_text = "".join(out) + tail_part
                if "i18n::tr_fmt" not in new_text:
                    if "use crate::*;" in new_text:
                        new_text = new_text.replace("use crate::*;", "use crate::*;\nuse nortex_ide::i18n::tr_fmt;", 1)
                    else:
                        new_text = new_text.replace("use crate::i18n::tr;", "use crate::i18n::{tr, tr_fmt};", 1)
                path.write_text(new_text, encoding="utf-8", newline="\n")
            print("##", path.relative_to(ROOT))
    for t in sorted(set(templates)):
        print(t)


if __name__ == "__main__":
    main()
