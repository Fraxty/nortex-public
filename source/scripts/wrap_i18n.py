"""Einmaliges Hilfsskript: umschließt UI-Literale in ide/src mit tr(...) und sammelt die Schlüssel."""
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent / "ide" / "src"
LIT = r'"((?:[^"\\\n]|\\.)*)"'
HAS_LETTER = re.compile(r"[A-Za-zÄÖÜäöüß]{2,}")

PATTERNS = [
    re.compile(r'(\.(?:button|label|heading|weak|strong|small_button|menu_button|collapsing|selectable_label|link|on_hover_text|hint_text|small|monospace)\(\s*)' + LIT),
    re.compile(r'((?:RichText|CollapsingHeader|Window|Label|Button|IndustrialButton)::new\((?:&?[a-z_]+,\s*)?)' + LIT),
    re.compile(r'(\.(?:checkbox)\([^"\n]*?,\s*)' + LIT),
    re.compile(r'(\.selectable_value\([^"\n]*?,\s*)' + LIT),
    re.compile(r'(\b(?:status|message|notice)\w*\s*=\s*)' + LIT + r'(?=\.into\(\))'),
    re.compile(r'(\.(?:text|add_filter|set_title|set_description)\(\s*)' + LIT),
]

collected = set()


def wrap_line(line: str, in_test: bool) -> str:
    for pattern in PATTERNS:
        def repl(match: re.Match) -> str:
            prefix, text = match.group(1), match.group(2)
            if "tr(" in prefix or not HAS_LETTER.search(text) or "{" in text:
                return match.group(0)
            collected.add(text)
            return f'{prefix}tr("{text}")'
        line = pattern.sub(repl, line)
    return line


def main() -> None:
    changed = []
    for path in sorted(ROOT.rglob("*.rs")):
        if path.name in ("i18n.rs", "translations.rs"):
            continue
        text = path.read_text(encoding="utf-8")
        in_lib = "shell" not in path.relative_to(ROOT).parts and path.name != "main.rs"
        lines = text.split("\n")
        out = []
        in_test = False
        for line in lines:
            if line.strip() == "#[cfg(test)]":
                in_test = True
            out.append(line if in_test else wrap_line(line, in_test))
        new = "\n".join(out)
        if new != text:
            if in_lib and "use crate::i18n::tr;" not in new:
                match = re.search(r"^use [^\n]*;\n", new, re.M)
                if match:
                    new = new[: match.start()] + "use crate::i18n::tr;\n" + new[match.start():]
                else:
                    sys.stderr.write(f"Kein use-Block: {path}\n")
            path.write_text(new, encoding="utf-8", newline="\n")
            changed.append(str(path))
    Path(sys.argv[1]).write_text(json.dumps(sorted(collected), ensure_ascii=False, indent=0), encoding="utf-8")
    print(len(collected), "Schlüssel,", len(changed), "Dateien")
    for item in changed:
        print(item)


main()
