# Kapitel 1 – Einführung

← [Inhaltsverzeichnis](README.md) · Weiter: [Kapitel 2](02-installation.md)

## Was ist NORTEX?

NORTEX ist eine statisch und streng typisierte, kompilierte Programmiersprache mit C#-ähnlicher Deklarationssyntax. Der Compiler `nortexc` ist in Rust geschrieben und erzeugt über Cranelift native Windows-x64-Programme (`.exe`).

```nortex
public int main() {
    return 42;
}
```

## Ziele

Laut Projektdokumentation: Speichersicherheit ohne Garbage Collector (Ownership/Borrowing), explizite Veränderlichkeit, stabile Diagnosecodes (`NXnnnn`) und ein kompilierter Ausführungspfad. Davon sind **Veränderlichkeitsregeln und Diagnosen implementiert**; Ownership-Prüfung ist teilweise vorhanden (siehe [Kapitel 9](09-memory-management.md)).

## Eigenschaften (belegt)

- Bezeichner und Schlüsselwörter sind case-sensitive; Bezeichner sind ASCII-only.
- Bindungen sind standardmäßig **unveränderlich**; `mut` macht sie veränderlich.
- Kein `null` in sicherem Code.
- Overflow-Prüfung bei Ganzzahlarithmetik standardmäßig aktiv.
- Deutsche Compilermeldungen mit stabilen Codes.

## Abgrenzung

NORTEX sieht C#-ähnlich aus, ist aber eigenständig: Top-Level-Funktionen ohne Klasse, `mut`-Bindungen, Ownership-Konzepte und ein eigener, deutlich kleinerer Funktionsumfang. Syntax anderer Sprachen darf nicht angenommen werden (z. B. keine Casts, kein `do-while`, kein `let`).

## Einsatzmöglichkeiten (aktuell realistisch)

Kleine Konsolenprogramme mit ganzzahliger Logik, Compiler-/Sprachexperimente, Lernprojekt. Allgemeine Anwendungsentwicklung ist erst nach Umsetzung von Strings, Structs, Heap und Standardbibliothek möglich (siehe [README](README.md#feature-übersicht-stand-der-implementierung)).

## Technische Voraussetzungen

Windows 10/11 x64, Rust-Toolchain (Edition 2024, Cargo ≥ 1.88) und die MSVC-Buildtools (Linker `link.exe`). Siehe [Kapitel 2](02-installation.md).
