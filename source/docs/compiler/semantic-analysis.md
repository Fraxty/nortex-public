# Semantische Analyse

Die semantische Analyse ist ein früher Validierungspass und noch kein vollständiger Typchecker oder Borrow Checker.

## Bereits teilweise umgesetzt

- doppelte Top-Level-Deklarationen und Membernamen
- bekannte Typnamen und generische Arität
- Case-sensitive Namen und PascalCase-/camelCase-Warnungen
- unbekannte Werte mit Diagnosecode `NX1004`
- unbekannte Typen mit `NX1100`
- erste lexikalische Blockscopes für Namensauflösung
- `const`-Initialisierung mit begrenzter Compile-Time-Ausdruckserkennung
- Zuweisungen an `const`-Bindungen und Parameterbindungen
- einfache `&mut`-bezogene Membermutationprüfung
- `break`-/`continue`-Kontextprüfung und `switch`-Default-Regeln
- höchstens eine erkannte Klassen-Basisklasse

## Noch nicht implementiert

- vollständige Ausdruckstypprüfung, Typinferenz und Literalbereichsprüfung
- Rückgabepfad-/Definite-Assignment-Analyse
- Konstruktorauflösung und Parameterzahl-/Argumenttypprüfung
- vollständige Sichtbarkeits- und Modulauflösung
- `must_use`, Match-Vollständigkeit und Trait-/Interface-Vertragsprüfung
- Move-/Borrow-/Lifetime-/Drop-Analyse
- Unsafe-Kontextprüfung, FFI-Regeln und vollständiger Ownership-Graph

Die vorhandene Namensauflösung ist keine vollständige Symboltabellen- und Typprüfung. Erfolgreiches `nortexc check` ist daher noch keine Garantie für Typkorrektheit.
