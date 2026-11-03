# NORTEX – Compiler-Typkatalog

Der Rust-Compiler stellt `type_system::PrimitiveType` als kanonischen Katalog primitiver Typen bereit. Parser, semantische Analyse und das spätere native Backend müssen diesen Katalog verwenden, statt voneinander abweichende Listen zu pflegen.

Unterstützte Layoutbeschreibungen sind `WindowsX64` und `WindowsArm64`. Beide Ziele verwenden 64-Bit-Pointer; `nint`, `nuint`, `isize` und `usize` besitzen daher auf diesen Targets 64 Bit. `bool` wird als ein Byte gespeichert, `char` als 32-Bit-Unicode-Skalarwert und `string` derzeit als interner Pointer-/Längen-/Kapazitätsdeskriptor mit 24 Byte beschrieben. Das Stringlayout ist noch keine stabile ABI-Zusage.

`i128`, `u128` und der Entwurfstyp `decimal` besitzen einen Layoutdeskriptor mit 16 Byte. Dezimalarithmetik, Rundung und Fehlerbehandlung sind noch nicht implementiert. `void` und `!` besitzen kein Wertlayout.

Es gibt keine zusätzlichen Aliasnamen `Int32`, `String`, `Bool`, `Unit` oder `Never`. Die Schreibweisen sind case-sensitive; `int`, `string`, `bool`, `void` und `!` sind die kanonischen Formen.

**Implementierungsgrenze:** Ein Layoutdeskriptor ist noch keine vollständige Typprüfung. Zuweisungskompatibilität, Konversionsregeln, generische Unifikation, ABI-Stabilität und Ownership-Prüfung sind separat umzusetzen und zu testen.

## Teilimplementierung des Ausdrucks-Typcheckers (0.3.5-dev)

`semantic/type_checker.rs` sammelt bekannte Funktions- und Klassenmember-Signaturen und prüft im ersten Schritt Variableninitialisierungen, Rückgaben, Bedingungen, unäre und binäre Operatoren, ternäre Ausdrücke, Aufrufe, Konstruktoren, Member, Indexzugriffe, `await` sowie `foreach`-Elementtypen. Überladungen werden für den unterstützten nicht-generischen Subset durch exakte Parametertypen und Argumentanzahl aufgelöst.

Dies ist noch kein vollständiger Typsystem- oder Namensauflösungspass: generische Substitution, Typvariablen, implizite/kontextuelle Literaltypisierung, Wertebereichsanalyse, statische Member-/Sichtbarkeitsprüfung, Interface-Mehrdeutigkeiten und Kontrollflussdatenfluss fehlen. Der Typchecker darf daher nicht als vollständige statische Typgarantie verstanden werden.
