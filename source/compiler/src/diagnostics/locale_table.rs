//! Englische Fassungen der Compiler-Meldungen (Schl?ssel: deutscher Text, `{}` = Platzhalter).
//! Nicht aufgef?hrte Meldungen bleiben deutsch.

pub(super) const TABLE: &[(&str, &str)] = &[
    (
        "Manifest '{}' nicht lesbar: {}",
        "Manifest '{}' is not readable: {}",
    ),
    (
        "Manifest '{}' ist beschädigt: {}",
        "Manifest '{}' is corrupt: {}",
    ),
    (
        "[package].name darf nicht leer sein",
        "[package].name must not be empty",
    ),
    (
        "[build].main darf nicht leer sein",
        "[build].main must not be empty",
    ),
    (
        "[build].target '{}' wird nicht unterstützt (erwartet: {})",
        "[build].target '{}' is not supported (expected: {})",
    ),
    (
        "[build].opt_level muss zwischen 0 und 3 liegen",
        "[build].opt_level must be between 0 and 3",
    ),
    (
        "Ausgabename '{}' muss ein einfacher Dateiname ohne Pfadteile sein",
        "Output name '{}' must be a simple file name without path components",
    ),
    (
        "NORTEX: Syntax und grundlegende Semantik geprüft ({} Tokens).",
        "NORTEX: syntax and basic semantics checked ({} tokens).",
    ),
    (
        "Quelldatei konnte nicht gelesen werden: {}",
        "Source file could not be read: {}",
    ),
    (
        "Ausgabeverzeichnis '{}' konnte nicht angelegt werden: {}",
        "Output directory '{}' could not be created: {}",
    ),
    (
        "COFF-Objekt '{}' konnte nicht geschrieben werden: {}",
        "COFF object '{}' could not be written: {}",
    ),
    (
        "Nicht unterstütztes Ausgabeformat für '{}'; erwartet wird .obj oder .exe.",
        "Unsupported output format for '{}'; expected .obj or .exe.",
    ),
    (
        "Die Option -o/--output darf nur einmal angegeben werden.",
        "The option -o/--output may only be given once.",
    ),
    (
        "Nach -o/--output muss ein Ausgabepfad stehen.",
        "An output path must follow -o/--output.",
    ),
    (
        "Die Option --emit-obj darf nur einmal angegeben werden.",
        "The option --emit-obj may only be given once.",
    ),
    (
        "Quelldatei für Diagnose-Snippets konnte nicht gelesen werden: {}",
        "Source file for diagnostic snippets could not be read: {}",
    ),
    (
        "Es gibt keine MIR-Funktionen für die Codegenerierung.",
        "There are no MIR functions for code generation.",
    ),
    (
        "Windows-x64-Target ist ungültig: {}",
        "Windows x64 target is invalid: {}",
    ),
    (
        "Cranelift unterstützt Windows x64 nicht: {}",
        "Cranelift does not support Windows x64: {}",
    ),
    (
        "Windows-x64-Codegenerator konnte nicht initialisiert werden: {}",
        "Windows x64 code generator could not be initialized: {}",
    ),
    (
        "COFF-Objekt konnte nicht initialisiert werden: {}",
        "COFF object could not be initialized: {}",
    ),
    (
        "Externe Funktion '{}' darf keinen string zurückgeben; C-Zeiger besitzen keinen Slice-Deskriptor.",
        "External function '{}' must not return a string; C pointers have no slice descriptor.",
    ),
    (
        "COFF-Symbol '{}' ist nicht eindeutig.",
        "COFF symbol '{}' is not unique.",
    ),
    (
        "Externe Funktion '{}' konnte nicht deklariert werden: {}",
        "External function '{}' could not be declared: {}",
    ),
    ("MIR {} ist ungültig: {}", "MIR {} is invalid: {}"),
    (
        "Windows-Programme benötigen derzeit genau eine parameterlose int-main-Funktion.",
        "Windows programs currently require exactly one parameterless int main function.",
    ),
    (
        "Funktion '{}' konnte nicht deklariert werden: {}",
        "Function '{}' could not be declared: {}",
    ),
    (
        "Runtime-Funktion für Konsolenausgabe konnte nicht deklariert werden: {}",
        "Runtime function for console output could not be declared: {}",
    ),
    (
        "Runtime-Funktion für String-Konkatenation konnte nicht deklariert werden: {}",
        "Runtime function for string concatenation could not be declared: {}",
    ),
    (
        "Runtime-Funktion für String-Vergleich nicht deklarierbar: {}",
        "Runtime function for string comparison cannot be declared: {}",
    ),
    (
        "Runtime-Funktion zur String-Freigabe nicht deklarierbar: {}",
        "Runtime function for releasing strings cannot be declared: {}",
    ),
    (
        "Mehr als eine Funktion wurde auf das Windows-Einstiegssymbol 'main' abgebildet.",
        "More than one function was mapped to the Windows entry symbol 'main'.",
    ),
    (
        "Globalvariable '{}' konnte nicht deklariert werden: {}",
        "Global variable '{}' could not be declared: {}",
    ),
    (
        "Initialwert der Globalvariable '{}' konnte nicht gespeichert werden: {}",
        "Initial value of global variable '{}' could not be stored: {}",
    ),
    (
        "Maschinencode für '{}' konnte nicht erzeugt werden: {}",
        "Machine code for '{}' could not be generated: {}",
    ),
    (
        "COFF-Objekt konnte nicht serialisiert werden: {}",
        "COFF object could not be serialized: {}",
    ),
    (
        "Feld- und Dereferenzierungszuweisungen sind im Codegen-Subset nicht unterstützt.",
        "Field and dereference assignments are not supported in the codegen subset.",
    ),
    (
        "void kann nicht als Zuweisungsziel verwendet werden.",
        "void cannot be used as an assignment target.",
    ),
    (
        "MIR-Deinit ist im Codegen-Subset nicht unterstützt.",
        "MIR deinit is not supported in the codegen subset.",
    ),
    (
        "Aufruf mit Cleanup-Kante wird im Codegen-Subset nicht unterstützt.",
        "Calls with a cleanup edge are not supported in the codegen subset.",
    ),
    (
        "Konsolenausgabe fehlt in der Runtime-Funktionsdeklaration.",
        "Console output is missing in the runtime function declaration.",
    ),
    (
        "Console.write/writeLine benötigt einen Wert vom Typ 'string'.",
        "Console.write/writeLine requires a value of type 'string'.",
    ),
    (
        "Unbekannte MIR-Funktion '{}'.",
        "Unknown MIR function '{}'.",
    ),
    (
        "Signatur für MIR-Funktion '{}' fehlt.",
        "Signature for MIR function '{}' is missing.",
    ),
    (
        "MIR-Drop ist im Codegen-Subset nicht unterstützt.",
        "MIR drop is not supported in the codegen subset.",
    ),
    (
        "String '{}' kann hier nicht kopiert werden; Strings besitzen ihren Speicher und werden verschoben.",
        "String '{}' cannot be copied here; strings own their memory and are moved.",
    ),
    (
        "String-Konkatenation fehlt in der Runtime-Funktionsdeklaration.",
        "String concatenation is missing in the runtime function declaration.",
    ),
    (
        "Stringvergleiche benötigen zwei Werte vom Typ 'string'.",
        "String comparisons require two values of type 'string'.",
    ),
    (
        "String-Vergleich fehlt in der Runtime-Funktionsdeklaration.",
        "String comparison is missing in the runtime function declaration.",
    ),
    (
        "Der Operator '+' verknüpft nur zwei Werte vom Typ 'string'.",
        "The operator '+' only concatenates two values of type 'string'.",
    ),
    (
        "Stringliteral überschreitet die native Slice-Länge.",
        "String literal exceeds the native slice length.",
    ),
    (
        "UTF-8-Stringdaten konnten nicht deklariert werden: {}",
        "UTF-8 string data could not be declared: {}",
    ),
    (
        "UTF-8-Stringdaten konnten nicht gespeichert werden: {}",
        "UTF-8 string data could not be stored: {}",
    ),
    (
        "String-Slice konnte nicht deklariert werden: {}",
        "String slice could not be declared: {}",
    ),
    (
        "String-Slice konnte nicht gespeichert werden: {}",
        "String slice could not be stored: {}",
    ),
    (
        "Stringwerte werden im Switch-Codegen nicht unterstützt.",
        "String values are not supported in switch code generation.",
    ),
    (
        "Switch-Discriminant hat Cranelift-Typ {}, erwartet wird {}.",
        "Switch discriminant has Cranelift type {}, expected {}.",
    ),
    (
        "Switch-Discriminant besitzt einen nicht unterstützten Operanden.",
        "Switch discriminant has an unsupported operand.",
    ),
    (
        "Stringoperatoren sind im nativen Codegen-Subset nicht unterstützt.",
        "String operators are not supported in the native codegen subset.",
    ),
    (
        "Referenzen, Aggregate und Discriminant sind im Codegen-Subset nicht unterstützt.",
        "References, aggregates and discriminants are not supported in the codegen subset.",
    ),
    (
        "Operand _L{} hat Cranelift-Typ {}, erwartet wird {}.",
        "Operand _L{} has Cranelift type {}, expected {}.",
    ),
    (
        "Feld- und Dereferenzierungsoperanden sind im Codegen-Subset nicht unterstützt.",
        "Field and dereference operands are not supported in the codegen subset.",
    ),
    (
        "Float- und String-Konstanten sind im Codegen-Subset nicht unterstützt.",
        "Float and string constants are not supported in the codegen subset.",
    ),
    (
        "Feld- und Dereferenzierungsoperanden besitzen im Codegen-Subset keinen Wertetyp.",
        "Field and dereference operands have no value type in the codegen subset.",
    ),
    (
        "MIR-Typ '{}' wird im Codegen-Subset nicht unterstützt.",
        "MIR type '{}' is not supported in the codegen subset.",
    ),
    (
        "Globalvariable '{}' besitzt einen int-Wert, aber den Typ bool.",
        "Global variable '{}' has an int value but the type bool.",
    ),
    (
        "Globalvariable '{}' besitzt einen string-Wert ohne Deskriptor.",
        "Global variable '{}' has a string value without a descriptor.",
    ),
    (
        "Globalvariable '{}' besitzt einen bool-Wert, aber einen numerischen Typ.",
        "Global variable '{}' has a bool value but a numeric type.",
    ),
    (
        "Die Quelle enthält keine Funktionskörper, die in MIR übersetzt werden können.",
        "The source contains no function bodies that can be translated to MIR.",
    ),
    (
        "Variablentyp-Inferenz wird in dieser Lowering-Stufe nicht unterstützt.",
        "Variable type inference is not supported in this lowering stage.",
    ),
    (
        "Diese Kontrollflussanweisung wird in der aktuellen Lowering-Stufe nicht unterstützt.",
        "This control-flow statement is not supported in the current lowering stage.",
    ),
    (
        "Dereferenzierte Zuweisungen benötigen eine lokale Referenz.",
        "Dereferenced assignments require a local reference.",
    ),
    (
        "Das MIR-Subset unterstützt derzeit nur direkte freie Funktionen und Console-Ausgabe.",
        "The MIR subset currently only supports direct free functions and console output.",
    ),
    (
        "Console.write und Console.writeLine benötigen genau einen Stringwert.",
        "Console.write and Console.writeLine require exactly one string value.",
    ),
    (
        "Funktion '{}' ist im MIR-Subset nicht auflösbar.",
        "Function '{}' cannot be resolved in the MIR subset.",
    ),
    (
        "Überladener Aufruf von '{}' ist im MIR-Subset nicht eindeutig auflösbar.",
        "Overloaded call of '{}' cannot be resolved unambiguously in the MIR subset.",
    ),
    (
        "Generische Funktion '{}' benötigt noch Monomorphisierung.",
        "Generic function '{}' still needs monomorphization.",
    ),
    (
        "Dieser Ausdruck wird in der aktuellen Lowering-Stufe nicht unterstützt.",
        "This expression is not supported in the current lowering stage.",
    ),
    (
        "Zeilenumbruch in String-Literal nicht erlaubt",
        "Line break in string literal is not allowed",
    ),
    (
        "Unicode-Zeichen '{}' ist in Bezeichnern und Operatoren nicht zulässig",
        "Unicode character '{}' is not allowed in identifiers and operators",
    ),
    (
        "Hexadezimales Literal benötigt mindestens eine Ziffer.",
        "A hexadecimal literal requires at least one digit.",
    ),
    (
        "Binäres Literal benötigt mindestens eine 0- oder 1-Ziffer.",
        "A binary literal requires at least one 0 or 1 digit.",
    ),
    (
        "Ein Binärliteral darf nur die Ziffern 0 und 1 enthalten.",
        "A binary literal may only contain the digits 0 and 1.",
    ),
    (
        "Exponent benötigt mindestens eine Ziffer.",
        "An exponent requires at least one digit.",
    ),
    (
        "Zeichenliteral muss genau einen Unicode-Skalarwert enthalten und mit einem Apostroph geschlossen werden.",
        "A character literal must contain exactly one Unicode scalar value and be closed with an apostrophe.",
    ),
    (
        "Der MSVC-Linker ist nur auf Windows verfügbar.",
        "The MSVC linker is only available on Windows.",
    ),
    (
        "MSVC-Linker konnte nicht gestartet werden: {}",
        "MSVC linker could not be started: {}",
    ),
    (
        "MSVC-Linker ist mit Exitcode {} fehlgeschlagen.\n{}{}",
        "MSVC linker failed with exit code {}.\n{}{}",
    ),
    (
        "Temporäre COFF-Datei '{}' konnte nicht entfernt werden: {}",
        "Temporary COFF file '{}' could not be removed: {}",
    ),
    (
        "Pfad des laufenden Compilers konnte nicht ermittelt werden: {}",
        "Path of the running compiler could not be determined: {}",
    ),
    (
        "NORTEX_RUNTIME_LIB zeigt nicht auf eine Datei: '{}'.",
        "NORTEX_RUNTIME_LIB does not point to a file: '{}'.",
    ),
    (
        "NORTEX Runtime-Import benötigt 'nortex_runtime.lib'. Baue zuerst `cargo build -p nortex-runtime` oder setze NORTEX_RUNTIME_LIB auf die Runtime-Bibliothek.",
        "The NORTEX runtime import requires 'nortex_runtime.lib'. Run `cargo build -p nortex-runtime` first or set NORTEX_RUNTIME_LIB to the runtime library.",
    ),
    (
        "Temporäre COFF-Datei konnte nicht geschrieben werden: {}",
        "Temporary COFF file could not be written: {}",
    ),
    (
        "Temporäre COFF-Datei konnte nicht geschrieben werden: {}; die Teildatei '{}' konnte nicht entfernt werden: {}",
        "Temporary COFF file could not be written: {}; the partial file '{}' could not be removed: {}",
    ),
    (
        "Temporäre COFF-Datei '{}' konnte nicht angelegt werden: {}",
        "Temporary COFF file '{}' could not be created: {}",
    ),
    (
        "Kein eindeutiger temporärer Name für die COFF-Datei verfügbar.",
        "No unique temporary name available for the COFF file.",
    ),
    (
        "Visual Studio Installer 'vswhere.exe' wurde nicht gefunden. Installiere die MSVC-x64-Buildtools.",
        "Visual Studio Installer 'vswhere.exe' was not found. Install the MSVC x64 build tools.",
    ),
    (
        "vswhere.exe konnte nicht ausgeführt werden: {}",
        "vswhere.exe could not be run: {}",
    ),
    (
        "vswhere.exe konnte keine MSVC-x64-Installation finden (Exitcode {}).",
        "vswhere.exe could not find an MSVC x64 installation (exit code {}).",
    ),
    (
        "MSVC-Linkerpfad konnte nicht ermittelt werden: {}",
        "MSVC linker path could not be determined: {}",
    ),
    (
        "vswhere.exe konnte link.exe nicht finden (Exitcode {}).",
        "vswhere.exe could not find link.exe (exit code {}).",
    ),
    (
        "MSVC-x64-Linker fehlt. Installiere die Komponente 'VC.Tools.x86.x64'.",
        "The MSVC x64 linker is missing. Install the component 'VC.Tools.x86.x64'.",
    ),
    (
        "MSVC-Umgebung konnte nicht initialisiert werden: {}",
        "MSVC environment could not be initialized: {}",
    ),
    (
        "vcvars64.bat ist mit Exitcode {} fehlgeschlagen.\n{}{}",
        "vcvars64.bat failed with exit code {}.\n{}{}",
    ),
    (
        "MSVC-Umgebungspfad '{}' konnte nicht für cmd.exe aufbereitet werden: {}",
        "MSVC environment path '{}' could not be prepared for cmd.exe: {}",
    ),
    (
        "MSVC-Kurzpfad für '{}' konnte nicht ermittelt werden.",
        "MSVC short path for '{}' could not be determined.",
    ),
    (
        "Globaler Typ '{}' wird in der aktuellen MIR-Stufe nicht unterstützt.",
        "Global type '{}' is not supported in the current MIR stage.",
    ),
    (
        "Der globale Initialisierungswert muss ein unterstützter skalarer Konstantenausdruck sein.",
        "The global initializer must be a supported scalar constant expression.",
    ),
    (
        "Ein string-Global benötigt ein Stringliteral und umgekehrt.",
        "A string global requires a string literal and vice versa.",
    ),
    (
        "Ein bool-Global benötigt einen bool-Ausdruck.",
        "A bool global requires a bool expression.",
    ),
    (
        "Ein numerisches Global benötigt einen Zahlen-Ausdruck.",
        "A numeric global requires a numeric expression.",
    ),
    (
        "Der Initialisierungswert liegt außerhalb des Wertebereichs von '{}'.",
        "The initializer is outside the value range of '{}'.",
    ),
    (
        "Lokaler Typ '{}' liegt außerhalb des skalaren MIR-Subsets.",
        "Local type '{}' is outside the scalar MIR subset.",
    ),
    (
        "Ein void-MIR-Rückgabepfad darf keinen Wert liefern.",
        "A void MIR return path must not provide a value.",
    ),
    (
        "Ein nicht-void-MIR-Rückgabepfad benötigt einen Wert.",
        "A non-void MIR return path requires a value.",
    ),
    (
        "Void-Aufrufe sind im MIR-Subset nur für Console.write und Console.writeLine verfügbar.",
        "Void calls in the MIR subset are only available for Console.write and Console.writeLine.",
    ),
    (
        "Console-Ausgabe benötigt einen Wert vom Typ 'string'.",
        "Console output requires a value of type 'string'.",
    ),
    (
        "Ein verworfener Ausdruck darf im MIR-Subset keinen Wert verschieben.",
        "A discarded expression must not move a value in the MIR subset.",
    ),
    (
        "Ein for-Inkrement darf im MIR-Subset keinen Wert verschieben.",
        "A for increment must not move a value in the MIR subset.",
    ),
    (
        "Lokale Bindung '{}' konnte beim MIR-Lowering nicht aufgelöst werden.",
        "Local binding '{}' could not be resolved during MIR lowering.",
    ),
    (
        "Integerliteral '{}' passt nicht in den MIR-Zieltyp i64.",
        "Integer literal '{}' does not fit the MIR target type i64.",
    ),
    (
        "Ein void-Funktionsaufruf kann nur als eigenständige Anweisung verwendet werden.",
        "A void function call can only be used as a standalone statement.",
    ),
    (
        "Zuweisungsziel '{}' konnte beim MIR-Lowering nicht aufgelöst werden.",
        "Assignment target '{}' could not be resolved during MIR lowering.",
    ),
    (
        "Zuweisung durch '{}' erfordert eine veränderliche Referenz.",
        "Assignment through '{}' requires a mutable reference.",
    ),
    (
        "Zuweisungsziel '{}' ist im MIR nicht veränderlich.",
        "Assignment target '{}' is not mutable in MIR.",
    ),
    (
        "Zuweisungsoperator '{}' wird im MIR-Subset nicht unterstützt.",
        "Assignment operator '{}' is not supported in the MIR subset.",
    ),
    (
        "Dereferenzierung ist nur für lokale Referenzbindungen unterstützt",
        "Dereferencing is only supported for local reference bindings",
    ),
    (
        "Referenz '{}' konnte beim Dereferenzieren nicht aufgelöst werden.",
        "Reference '{}' could not be resolved when dereferencing.",
    ),
    (
        "Dereferenzierung erwartet eine lokale Referenz",
        "Dereferencing expects a local reference",
    ),
    (
        "Ein Borrow-Ausdruck benötigt ein Referenzziel.",
        "A borrow expression requires a reference target.",
    ),
    (
        "Die Zweige eines bedingten MIR-Ausdrucks müssen denselben skalaren Typ besitzen.",
        "The branches of a conditional MIR expression must have the same scalar type.",
    ),
    (
        "Nicht-void-Funktion erreicht beim MIR-Lowering das Ende ohne return.",
        "A non-void function reaches the end without return during MIR lowering.",
    ),
    (
        "Operator '{}' wird im aktuellen MIR-Subset nicht unterstützt.",
        "Operator '{}' is not supported in the current MIR subset.",
    ),
    (
        "Nach dem Namespace wird ';' erwartet.",
        "A ';' is expected after the namespace.",
    ),
    (
        "Nach einer using-Direktive wird ';' erwartet.",
        "A ';' is expected after a using directive.",
    ),
    (
        "'let' ist reserviert, aber in NORTEX nicht unterstützt. Verwende eine Typangabe für Deklarationen.",
        "'let' is reserved but not supported in NORTEX. Use a type annotation for declarations.",
    ),
    (
        "Nach dem Rückgabetyp wird ein Funktionsname erwartet.",
        "A function name is expected after the return type.",
    ),
    (
        "Nach einer globalen Variable wird ';' erwartet.",
        "A ';' is expected after a global variable.",
    ),
    (
        "Ungültige Top-Level-Deklaration. Erwartet wird eine Funktion mit '(' oder eine globale Variable mit '=' bzw. ';'.",
        "Invalid top-level declaration. Expected a function with '(' or a global variable with '=' or ';'.",
    ),
    (
        "Nach dem Typ-Schlüsselwort wird ein Name erwartet.",
        "A name is expected after the type keyword.",
    ),
    (
        "Vor dem Typkörper wird '{' erwartet.",
        "A '{' is expected before the type body.",
    ),
    (
        "Typkörper ist nicht abgeschlossen.",
        "Type body is not terminated.",
    ),
    (
        "Nach den Enum-Variantentypen wird ')' erwartet.",
        "A ')' is expected after the enum variant types.",
    ),
    (
        "Nach 'operator' wird ein Operatorzeichen erwartet.",
        "An operator symbol is expected after 'operator'.",
    ),
    (
        "Nach dem Typ wird ein Membername erwartet.",
        "A member name is expected after the type.",
    ),
    (
        "Nach einem Feld wird ';' erwartet.",
        "A ';' is expected after a field.",
    ),
    (
        "Vor den Parametern wird '(' erwartet.",
        "A '(' is expected before the parameters.",
    ),
    (
        "Nach dem Parametertyp wird ein Name erwartet.",
        "A name is expected after the parameter type.",
    ),
    (
        "Nach den Parametern wird ')' erwartet.",
        "A ')' is expected after the parameters.",
    ),
    (
        "Generische Parameter benötigen ein abschließendes '>'.",
        "Generic parameters require a closing '>'.",
    ),
    (
        "Ein Raw-Pointer benötigt 'const' oder 'mut'.",
        "A raw pointer requires 'const' or 'mut'.",
    ),
    (
        "In einem festen Arraytyp wird ';' zwischen Typ und Länge erwartet.",
        "A ';' is expected between type and length in a fixed array type.",
    ),
    (
        "Die Länge eines festen Arrays muss eine nichtnegative ganzzahlige Compile-Time-Konstante sein.",
        "The length of a fixed array must be a non-negative integer compile-time constant.",
    ),
    (
        "Ein fester Arraytyp benötigt ein abschließendes ']'.",
        "A fixed array type requires a closing ']'.",
    ),
    (
        "Nach dem Punkt wird ein weiterer Typname erwartet.",
        "Another type name is expected after the dot.",
    ),
    (
        "Generische Typargumente benötigen ein abschließendes '>'.",
        "Generic type arguments require a closing '>'.",
    ),
    (
        "Nach '[' wird ']' erwartet.",
        "A ']' is expected after '['.",
    ),
    ("Block ist nicht abgeschlossen.", "Block is not terminated."),
    (
        "'let' ist reserviert, aber in NORTEX nicht unterstützt. Verwende eine Typangabe, z. B. 'int value = 1;'.",
        "'let' is reserved but not supported in NORTEX. Use a type annotation, e.g. 'int value = 1;'.",
    ),
    (
        "Nach return wird ';' erwartet.",
        "A ';' is expected after return.",
    ),
    (
        "Nach throw wird ';' erwartet.",
        "A ';' is expected after throw.",
    ),
    ("Nach if wird '(' erwartet.", "A '(' is expected after if."),
    (
        "Nach der if-Bedingung wird ')' erwartet.",
        "A ')' is expected after the if condition.",
    ),
    (
        "Nach while wird '(' erwartet.",
        "A '(' is expected after while.",
    ),
    (
        "Nach der while-Bedingung wird ')' erwartet.",
        "A ')' is expected after the while condition.",
    ),
    (
        "Nach break wird ';' erwartet.",
        "A ';' is expected after break.",
    ),
    (
        "Nach continue wird ';' erwartet.",
        "A ';' is expected after continue.",
    ),
    (
        "Nach einem Ausdruck wird ';' erwartet.",
        "A ';' is expected after an expression.",
    ),
    (
        "Nach catch-Parameter wird ')' erwartet.",
        "A ')' is expected after the catch parameter.",
    ),
    (
        "Auf try muss mindestens catch oder finally folgen.",
        "A try must be followed by at least catch or finally.",
    ),
    (
        "Nach for wird '(' erwartet.",
        "A '(' is expected after for.",
    ),
    (
        "Nach dem foreach-Typ wird ein Variablenname erwartet.",
        "A variable name is expected after the foreach type.",
    ),
    (
        "Zwischen foreach-Variable und Sequenz wird 'in' erwartet.",
        "'in' is expected between the foreach variable and the sequence.",
    ),
    (
        "Nach der foreach-Sequenz wird ')' erwartet.",
        "A ')' is expected after the foreach sequence.",
    ),
    (
        "Nach der for-Initialisierung wird ';' erwartet.",
        "A ';' is expected after the for initializer.",
    ),
    (
        "Nach der for-Bedingung wird ';' erwartet.",
        "A ';' is expected after the for condition.",
    ),
    (
        "Nach dem for-Header wird ')' erwartet.",
        "A ')' is expected after the for header.",
    ),
    (
        "Nach switch wird '(' erwartet.",
        "A '(' is expected after switch.",
    ),
    (
        "Nach dem switch-Ausdruck wird ')' erwartet.",
        "A ')' is expected after the switch expression.",
    ),
    (
        "Vor den switch-Fällen wird '{' erwartet.",
        "A '{' is expected before the switch cases.",
    ),
    (
        "In einem switch-Block wird 'case' oder 'default' erwartet.",
        "'case' or 'default' is expected in a switch block.",
    ),
    (
        "Nach einem switch-Label wird ':' erwartet.",
        "A ':' is expected after a switch label.",
    ),
    (
        "Switch-Block ist nicht abgeschlossen.",
        "Switch block is not terminated.",
    ),
    (
        "Nach match wird '(' erwartet.",
        "A '(' is expected after match.",
    ),
    (
        "Nach dem Match-Ausdruck wird ')' erwartet.",
        "A ')' is expected after the match expression.",
    ),
    (
        "Vor den Match-Armen wird '{' erwartet.",
        "A '{' is expected before the match arms.",
    ),
    (
        "Nach dem Enum-Namen wird eine Variante erwartet.",
        "A variant is expected after the enum name.",
    ),
    (
        "Nach dem Match-Bindungsnamen wird ')' erwartet.",
        "A ')' is expected after the match binding name.",
    ),
    (
        "Nach dem Match-Pattern wird '=>' erwartet.",
        "A '=>' is expected after the match pattern.",
    ),
    (
        "Nach einem Ausdruck in einem Match-Arm wird ';' erwartet.",
        "A ';' is expected after an expression in a match arm.",
    ),
    (
        "Match-Block ist nicht abgeschlossen.",
        "Match block is not terminated.",
    ),
    (
        "Nach var wird ein Variablenname erwartet.",
        "A variable name is expected after var.",
    ),
    (
        "Nach dem Typ wird ein Variablenname erwartet.",
        "A variable name is expected after the type.",
    ),
    (
        "Nach einer Variablendeklaration wird ';' erwartet.",
        "A ';' is expected after a variable declaration.",
    ),
    (
        "Nach Funktionsargumenten wird ')' erwartet.",
        "A ')' is expected after function arguments.",
    ),
    (
        "Nach '.' wird ein Membername erwartet.",
        "A member name is expected after '.'.",
    ),
    (
        "Nach einem Index wird ']' erwartet.",
        "A ']' is expected after an index.",
    ),
    (
        "Ein '?' beginnt in NORTEX den Ternäroperator und benötigt ':'. Es gibt keinen '?'-Fehlerweitergabeoperator.",
        "In NORTEX a '?' starts the ternary operator and requires ':'. There is no '?' error propagation operator.",
    ),
    (
        "Nach dem wahren Ausdruck des Ternäroperators wird ':' erwartet.",
        "A ':' is expected after the true expression of the ternary operator.",
    ),
    (
        "Nach Konstruktorargumenten wird ')' erwartet.",
        "A ')' is expected after constructor arguments.",
    ),
    (
        "Nach generischen Aufrufargumenten wird ')' erwartet.",
        "A ')' is expected after generic call arguments.",
    ),
    (
        "Schlüsselwort ist an dieser Stelle kein Ausdruck.",
        "A keyword is not an expression at this position.",
    ),
    (
        "Nach einem gruppierten Ausdruck wird ')' erwartet.",
        "A ')' is expected after a grouped expression.",
    ),
    (
        "Nach '.' wird ein Bezeichner erwartet.",
        "An identifier is expected after '.'.",
    ),
    (
        "'mut' kennzeichnet Variablen, nicht Funktionen.",
        "'mut' marks variables, not functions.",
    ),
    (
        "abstract, extern und intrinsic schließen sich gegenseitig aus.",
        "abstract, extern and intrinsic are mutually exclusive.",
    ),
    (
        "Freie Funktionen dürfen nicht abstract sein.",
        "Free functions must not be abstract.",
    ),
    (
        "Die ABI \"{}\" wird nicht unterstützt; verwende extern \"C\".",
        "The ABI \"{}\" is not supported; use extern \"C\".",
    ),
    (
        "extern-Deklarationen benötigen eine explizite ABI-Angabe: extern \"C\".",
        "extern declarations require an explicit ABI: extern \"C\".",
    ),
    (
        "intrinsic ist ausschließlich in compilerverwalteten Dateien der NORTEX-Standardbibliothek zulässig.",
        "intrinsic is only allowed in compiler-managed files of the NORTEX standard library.",
    ),
    (
        "Eine intrinsic-Funktion darf in dieser Sprachversion nicht async sein.",
        "An intrinsic function must not be async in this language version.",
    ),
    (
        "Funktion '{}' benötigt einen Implementierungsblock.",
        "Function '{}' requires an implementation block.",
    ),
    (
        "abstract-, extern- und intrinsic-Funktionen dürfen keinen Implementierungsblock besitzen.",
        "abstract, extern and intrinsic functions must not have an implementation block.",
    ),
    (
        "Eine globale Variable kann nicht gleichzeitig mut und const sein.",
        "A global variable cannot be both mut and const.",
    ),
    (
        "Eine globale Variable benötigt einen Initialisierungswert.",
        "A global variable requires an initializer.",
    ),
    (
        "Der Initialisierungswert einer globalen Variable muss zur Compile-Zeit auswertbar sein.",
        "The initializer of a global variable must be evaluable at compile time.",
    ),
    (
        "Namespace '{}' kann nicht aufgelöst werden. Der Import ist weder Teil der NORTEX-Standardbibliothek noch im aktuellen Modul deklariert.",
        "Namespace '{}' cannot be resolved. The import is neither part of the NORTEX standard library nor declared in the current module.",
    ),
    (
        "Prüfe den Namespace-Pfad und die Schreibweise.",
        "Check the namespace path and spelling.",
    ),
    (
        "Der Modifikator '{}' darf nicht mehrfach angegeben werden.",
        "The modifier '{}' must not be specified more than once.",
    ),
    (
        "Konkrete Klasse '{}' implementiert die abstrakte Methode '{}' nicht.",
        "Concrete class '{}' does not implement the abstract method '{}'.",
    ),
    (
        "Die abstrakte Methode ist hier deklariert: Zeile {}.",
        "The abstract method is declared here: line {}.",
    ),
    (
        "Methode '{}' ist als override markiert, überschreibt aber keine virtual- oder abstract-Methode.",
        "Method '{}' is marked override but does not override a virtual or abstract method.",
    ),
    (
        "Methode '{}' überschreibt ein virtuelles Member und muss override deklarieren.",
        "Method '{}' overrides a virtual member and must be declared override.",
    ),
    (
        "Abstrakte Klasse '{}' kann nicht instanziiert werden.",
        "Abstract class '{}' cannot be instantiated.",
    ),
    (
        "'mut' kennzeichnet Variablen, nicht Typdeklarationen.",
        "'mut' marks variables, not type declarations.",
    ),
    (
        "'mut' kennzeichnet lokale oder globale Variablen, nicht Klassenmember.",
        "'mut' marks local or global variables, not class members.",
    ),
    (
        "Generischer Parameter '{}' ist mehrfach definiert.",
        "Generic parameter '{}' is defined more than once.",
    ),
    (
        "Nur Klassen dürfen von einer Basisklasse erben; Structs, Records, Interfaces und Traits dürfen keine Klasse erweitern.",
        "Only classes may inherit from a base class; structs, records, interfaces and traits must not extend a class.",
    ),
    (
        "Ein Struct, Record oder Enum kann nicht als Basisklasse oder Implementierungsvertrag verwendet werden.",
        "A struct, record or enum cannot be used as a base class or implementation contract.",
    ),
    (
        "Mehrfachvererbung von Klassen ist nicht zulässig. Eine Klasse darf höchstens eine Basisklasse besitzen.",
        "Multiple inheritance of classes is not allowed. A class may have at most one base class.",
    ),
    (
        "Interfaces enthalten nur Methodensignaturen und dürfen keine Felder deklarieren.",
        "Interfaces contain only method signatures and must not declare fields.",
    ),
    (
        "Der Initialisierungswert eines const-Feldes muss zur Compile-Zeit auswertbar sein.",
        "The initializer of a const field must be evaluable at compile time.",
    ),
    (
        "Ein const-Feld benötigt einen Initialisierungswert.",
        "A const field requires an initializer.",
    ),
    (
        "Interface-Methoden definieren nur Signaturen und dürfen keinen Implementierungsblock besitzen.",
        "Interface methods define only signatures and must not have an implementation block.",
    ),
    (
        "Methode '{}' benötigt einen Implementierungsblock. Signaturen ohne Rumpf müssen als abstract oder extern deklariert werden; intrinsic ist nur für die Standardbibliothek zulässig.",
        "Method '{}' requires an implementation block. Signatures without a body must be declared abstract or extern; intrinsic is only allowed for the standard library.",
    ),
    (
        "abstract-, extern- und intrinsic-Methoden dürfen keinen Implementierungsblock besitzen.",
        "abstract, extern and intrinsic methods must not have an implementation block.",
    ),
    (
        "extern-Deklarationen benötigen eine explizite ABI-Angabe. Die Syntax extern \"C\" wird noch nicht unterstützt.",
        "extern declarations require an explicit ABI. The syntax extern \"C\" is not supported yet.",
    ),
    (
        "Interface- und Trait-Verträge dürfen weder extern noch intrinsic sein.",
        "Interface and trait contracts must be neither extern nor intrinsic.",
    ),
    (
        "Eine intrinsic-Methode darf in dieser Sprachversion nicht async sein.",
        "An intrinsic method must not be async in this language version.",
    ),
    (
        "Eine abstrakte Methode darf nur in einer abstract-Klasse stehen.",
        "An abstract method may only appear in an abstract class.",
    ),
    (
        "Eine abstrakte Methode darf weder private noch static sein.",
        "An abstract method must be neither private nor static.",
    ),
    (
        "Methode und Feld dürfen nicht denselben Bezeichner '{}' verwenden.",
        "A method and a field must not use the same identifier '{}'.",
    ),
    (
        "Operatorimplementierungen sind in Interfaces nicht zulässig.",
        "Operator implementations are not allowed in interfaces.",
    ),
    (
        "Der Operator '{}' darf in NORTEX nicht überladen werden. Erlaubt sind ausschließlich ==, !=, <, >, <= und >=.",
        "The operator '{}' must not be overloaded in NORTEX. Only ==, !=, <, >, <= and >= are allowed.",
    ),
    (
        "Ein binärer Vergleichsoperator benötigt genau zwei Parameter.",
        "A binary comparison operator requires exactly two parameters.",
    ),
    (
        "Eine Operatorüberladung muss als static deklariert werden.",
        "An operator overload must be declared static.",
    ),
    (
        "Unbekannter Bezeichner '{}'. Der Wert wurde weder deklariert noch importiert.",
        "Unknown identifier '{}'. The value was neither declared nor imported.",
    ),
    (
        "Prüfe die Schreibweise oder deklariere den Wert vor seiner Verwendung.",
        "Check the spelling or declare the value before using it.",
    ),
    (
        "Unbekannter Typ '{}' in Konstruktoraufruf.",
        "Unknown type '{}' in constructor call.",
    ),
    (
        "Unbekannter generischer Typ '{}'.",
        "Unknown generic type '{}'.",
    ),
    (
        "Ein match darf höchstens einen '_' -Fallback-Arm besitzen.",
        "A match may have at most one '_' fallback arm.",
    ),
    (
        "Der '_' -Fallback-Arm sollte als letzter match-Arm stehen.",
        "The '_' fallback arm should be the last match arm.",
    ),
    (
        "Die Enum-Variante '{}' wird im match mehrfach behandelt.",
        "The enum variant '{}' is handled more than once in the match.",
    ),
    (
        "Match-Bindung '{}' ist bereits im Gültigkeitsbereich definiert.",
        "Match binding '{}' is already defined in scope.",
    ),
    (
        "Die Veränderung über ein Member oder einen Index erfordert einen exklusiven &mut-Borrow; '{}' ist hier nur unveränderlich ausgeliehen.",
        "Modifying through a member or an index requires an exclusive &mut borrow; '{}' is only borrowed immutably here.",
    ),
    (
        "Der unveränderliche Wert '{}' darf nicht neu zugewiesen werden.",
        "The immutable value '{}' must not be reassigned.",
    ),
    (
        "Das unveränderliche Feld '{}' darf nicht verändert werden.",
        "The immutable field '{}' must not be modified.",
    ),
    (
        "Der unveränderliche Wert '{}' kann nicht exklusiv ausgeliehen werden; deklariere ihn mit 'mut'.",
        "The immutable value '{}' cannot be borrowed exclusively; declare it with 'mut'.",
    ),
    (
        "Der unveränderliche Wert '{}' darf nicht inkrementiert oder dekrementiert werden.",
        "The immutable value '{}' must not be incremented or decremented.",
    ),
    (
        "Das null-Literal ist in sicherem NORTEX-Code nicht zulässig. Verwende Option<T>, Some(...) oder None.",
        "The null literal is not allowed in safe NORTEX code. Use Option<T>, Some(...) or None.",
    ),
    (
        "Typ '{}' erwartet {} generische Argumente, erhalten: {}.",
        "Type '{}' expects {} generic arguments, got: {}.",
    ),
    (
        "Unbekannter Typ '{}'. Typen müssen in NORTEX definiert oder importiert sein.",
        "Unknown type '{}'. Types must be defined or imported in NORTEX.",
    ),
    (
        "Unbekannter Typ '{}'. Meintest du '{}'?",
        "Unknown type '{}'. Did you mean '{}'?",
    ),
    (
        "Der Wertliteral-Typ '{}' ist nicht mit dem erwarteten Typ '{}' kompatibel.",
        "The value literal type '{}' is not compatible with the expected type '{}'.",
    ),
    (
        "Passe den deklarierten Typ oder das Literal an. NORTEX führt keine implizite Umwandlung zwischen unterschiedlichen primitiven Kategorien durch.",
        "Adjust the declared type or the literal. NORTEX performs no implicit conversion between different primitive categories.",
    ),
    (
        "Der Typ '{}' kann nicht als Variablentyp verwendet werden.",
        "The type '{}' cannot be used as a variable type.",
    ),
    (
        "Eine Funktion mit Rückgabetyp benötigt in diesem return einen Rückgabewert.",
        "A function with a return type requires a return value in this return.",
    ),
    (
        "Der Typname '{}' ist bereits durch NORTEX vordefiniert und kann nicht erneut deklariert werden.",
        "The type name '{}' is already predefined by NORTEX and cannot be declared again.",
    ),
    (
        "Modul '{}' ist nicht lesbar: {}",
        "Module '{}' is not readable: {}",
    ),
    (
        "Eine Variable ohne Typ benötigt einen Initialisierungswert für die Typinferenz.",
        "A variable without a type requires an initializer for type inference.",
    ),
    (
        "Bezeichner '{}' ist in diesem Gültigkeitsbereich bereits definiert.",
        "Identifier '{}' is already defined in this scope.",
    ),
    (
        "Eine Compile-Time-Konstante benötigt einen Initialisierungswert.",
        "A compile-time constant requires an initializer.",
    ),
    (
        "Der Initialisierungswert einer const-Konstante muss zur Compile-Zeit auswertbar sein.",
        "The initializer of a const constant must be evaluable at compile time.",
    ),
    (
        "Eine unveränderliche Variable benötigt einen Initialisierungswert.",
        "An immutable variable requires an initializer.",
    ),
    (
        "Foreach-Variable '{}' ist bereits im Gültigkeitsbereich definiert.",
        "Foreach variable '{}' is already defined in scope.",
    ),
    (
        "Ein default-Fall muss der letzte switch-Fall sein.",
        "A default case must be the last switch case.",
    ),
    (
        "Ein switch darf höchstens einen default-Fall besitzen.",
        "A switch may have at most one default case.",
    ),
    (
        "Der default-Fall sollte am Ende des switch stehen.",
        "The default case should be at the end of the switch.",
    ),
    (
        "Der &mut-Parameter '{}' ist als Bindung unveränderlich und darf nicht neu zugewiesen werden.",
        "The &mut parameter '{}' is an immutable binding and must not be reassigned.",
    ),
    (
        "Der &mut-Parameter '{}' ist als Bindung unveränderlich und darf nicht inkrementiert oder dekrementiert werden.",
        "The &mut parameter '{}' is an immutable binding and must not be incremented or decremented.",
    ),
    (
        "break ist nur innerhalb einer Schleife oder eines switch-Blocks zulässig.",
        "break is only allowed inside a loop or a switch block.",
    ),
    (
        "continue ist nur innerhalb einer Schleife zulässig.",
        "continue is only allowed inside a loop.",
    ),
    (
        "Typ '{}' erwartet {} generische Argumente, erhalten wurden {}.",
        "Type '{}' expects {} generic arguments, got {}.",
    ),
    (
        "Der Wert '{}' ist nicht aufrufbar.",
        "The value '{}' is not callable.",
    ),
    (
        "Methode '{}.{}' ist außerhalb ihres deklarierenden Typs nicht zugänglich.",
        "Method '{}.{}' is not accessible outside its declaring type.",
    ),
    (
        "Typ '{}' besitzt keine Methode '{}'.",
        "Type '{}' has no method '{}'.",
    ),
    (
        "'{}<{}>.{}' erwartet genau ein Argument vom Typ '{}', erhalten wurde '{}'.",
        "'{}<{}>.{}' expects exactly one argument of type '{}', got '{}'.",
    ),
    (
        "Feld '{}.{}' ist außerhalb seines deklarierenden Typs nicht zugänglich.",
        "Field '{}.{}' is not accessible outside its declaring type.",
    ),
    (
        "Typ '{}' besitzt kein Feld '{}'.",
        "Type '{}' has no field '{}'.",
    ),
    (
        "Instanzmethode '{}.{}' kann nicht statisch aufgerufen werden.",
        "Instance method '{}.{}' cannot be called statically.",
    ),
    (
        "Typ '{}' besitzt keine statische Methode '{}'.",
        "Type '{}' has no static method '{}'.",
    ),
    (
        "Speicherort '{}' kann in einem Aufruf nicht gleichzeitig geteilt und exklusiv ausgeliehen werden.",
        "Location '{}' cannot be both shared and exclusively borrowed in one call.",
    ),
    (
        "Der Aufruf von '{}' ist mehrdeutig.",
        "The call to '{}' is ambiguous.",
    ),
    (
        "Typ '{}' besitzt keinen Konstruktor mit {} Argumenten.",
        "Type '{}' has no constructor with {} arguments.",
    ),
    (
        "Eine Funktion mit Rückgabewert benötigt an dieser Stelle einen return-Ausdruck.",
        "A function with a return value requires a return expression here.",
    ),
    (
        "Der Ausdruck ist nicht iterierbar. Erwartet wird ein Array oder Iterator<T>.",
        "The expression is not iterable. An array or Iterator<T> is expected.",
    ),
    (
        "Das Result-Ergebnis wird verworfen; behandle Ok und Err explizit.",
        "The Result is discarded; handle Ok and Err explicitly.",
    ),
    (
        "Der match-Ausdruck ist nicht vollständig; fehlende Varianten: {}.",
        "The match expression is not exhaustive; missing variants: {}.",
    ),
    (
        "Das Literal {} liegt außerhalb des Wertebereichs von '{}'.",
        "The literal {} is outside the value range of '{}'.",
    ),
    (
        "Das Literal {} ist zu groß für jeden ganzzahligen Typ.",
        "The literal {} is too large for any integer type.",
    ),
    (
        "Unbekannter Typ '{}'. Meintest du einen der kanonischen NORTEX-Typen?",
        "Unknown type '{}'. Did you mean one of the canonical NORTEX types?",
    ),
    (
        "Der Wert '{}' wurde bereits verschoben und kann nicht erneut verwendet werden.",
        "The value '{}' has already been moved and cannot be used again.",
    ),
    (
        "await ist nur innerhalb einer async-Funktion zulässig.",
        "await is only allowed inside an async function.",
    ),
    (
        "Operator '{}' benötigt einen numerischen Wert, erhalten wurde '{}'.",
        "Operator '{}' requires a numeric value, got '{}'.",
    ),
    (
        "Die Operanden von '{}' müssen kompatible Zahlen, char oder string sein.",
        "The operands of '{}' must be compatible numbers, char or string.",
    ),
    (
        "Die Operanden von '{}' müssen denselben numerischen Typ besitzen; erhalten wurden '{}' und '{}'.",
        "The operands of '{}' must have the same numeric type; got '{}' and '{}'.",
    ),
    (
        "Ein Index benötigt einen ganzzahligen Typ, erhalten wurde '{}'.",
        "An index requires an integer type, got '{}'.",
    ),
    (
        "Typ '{}' unterstützt keinen Indexzugriff.",
        "Type '{}' does not support index access.",
    ),
    (
        "Linken nach '{}' fehlgeschlagen: {}",
        "Linking to '{}' failed: {}",
    ),
    (
        "Codegenerierung fehlgeschlagen: {}",
        "Code generation failed: {}",
    ),
    (
        "Unbekanntes Build-Argument: {}",
        "Unknown build argument: {}",
    ),
    (
        "--emit-obj erfordert einen Ausgabepfad mit .obj oder .o.",
        "--emit-obj requires an output path ending in .obj or .o.",
    ),
    (
        "NORTEX: Ausgabe geschrieben: {}",
        "NORTEX: Output written: {}",
    ),
    (
        "Verwendung: nortexc build <datei.nx> [-o|--output <datei.exe|datei.obj>] [--emit-obj] [--overflow-checks[=true|false]]",
        "Usage: nortexc build <file.nx> [-o|--output <file.exe|file.obj>] [--emit-obj] [--overflow-checks[=true|false]]",
    ),
    (
        "Verwendung: nortexc build <datei.nx> [-o|--output <datei.exe|datei.obj>] [--emit-obj] [--overflow-checks[=true|false]] | nortexc <datei.nx> [-o|--output <datei.exe|datei.obj>] | check <datei.nx> | mir <datei.nx> | --version",
        "Usage: nortexc build <file.nx> [-o|--output <file.exe|file.obj>] [--emit-obj] [--overflow-checks[=true|false]] | nortexc <file.nx> [-o|--output <file.exe|file.obj>] | check <file.nx> | mir <file.nx> | --version",
    ),
    (
        "MIR-Funktionen besitzen inkonsistente globale Variablen.",
        "MIR functions have inconsistent global variables.",
    ),
    (
        "Globalvariable '{}' fehlt im COFF-Modul.",
        "Global variable '{}' is missing from the COFF module.",
    ),
    (
        "Globalvariable '{}' besitzt keinen Werttyp.",
        "Global variable '{}' has no value type.",
    ),
    (
        "MIR-BB{} besitzt keinen Terminator.",
        "MIR-BB{} has no terminator.",
    ),
    (
        "Console.write/writeLine ben?tigt genau einen Stringwert.",
        "Console.write/writeLine requires exactly one string value.",
    ),
    (
        "Stringliteral '{}' fehlt im COFF-Datenpool.",
        "String literal '{}' is missing from the COFF data pool.",
    ),
    (
        "void besitzt keinen Cranelift-Werttyp.",
        "void has no Cranelift value type.",
    ),
    (
        "Globale Variablen ben?tigen einen Initialisierungswert.",
        "Global variables require an initializer.",
    ),
    (
        "Zuweisungen sind nur an lokale Namen oder dereferenzierte Referenzen zul?ssig.",
        "Assignments are only allowed to local names or dereferenced references.",
    ),
    (
        "Generische Funktionsaufrufe ben?tigen noch Monomorphisierung.",
        "Generic function calls still require monomorphization.",
    ),
    (
        "Aufruf von '{}' hat {} Argumente, aber keine unterst?tzte Signatur.",
        "Call to '{}' has {} arguments but no supported signature.",
    ),
    (
        "R?ckgabetyp '{}' liegt au?erhalb des skalaren MIR-Subsets.",
        "Return type '{}' is outside the scalar MIR subset.",
    ),
    (
        "Parametertyp '{}' liegt au?erhalb des skalaren MIR-Subsets.",
        "Parameter type '{}' is outside the scalar MIR subset.",
    ),
    (
        "break/continue besitzt beim MIR-Lowering kein umgebendes Loop-Ziel.",
        "break/continue has no enclosing loop target during MIR lowering.",
    ),
    (
        "Console-Ausgabe ben?tigt genau einen Stringwert.",
        "Console output requires exactly one string value.",
    ),
    (
        "Borrow-Ausdr?cke d?rfen nur lokale Werte oder dereferenzierte Referenzen referenzieren.",
        "Borrow expressions may only reference local values or dereferenced references.",
    ),
    ("Enum-Variante erwartet.", "Enum variant expected."),
    (
        "Generischer Parametername erwartet.",
        "Generic parameter name expected.",
    ),
    ("Typname erwartet.", "Type name expected."),
    ("Blockbeginn '{' erwartet.", "Start of block '{' expected."),
    (
        "Match-Variante oder '_' erwartet.",
        "Match variant or '_' expected.",
    ),
    (
        "Bindungsname im Match-Arm erwartet.",
        "Binding name in match arm expected.",
    ),
    ("Ausdruck erwartet.", "Expression expected."),
    ("Bezeichner erwartet.", "Identifier expected."),
    (
        "Nur Klassen d?rfen als abstract deklariert werden.",
        "Only classes may be declared abstract.",
    ),
    (
        "Raw-Pointer-Felder m?ssen ausdr?cklich als unsafe deklariert werden.",
        "Raw pointer fields must be explicitly declared unsafe.",
    ),
    (
        "Konstruktoren d?rfen weder abstract, extern noch intrinsic sein.",
        "Constructors may be neither abstract, extern nor intrinsic.",
    ),
    (
        "Enum-Varianten sind nur innerhalb von enum-Deklarationen zul?ssig.",
        "Enum variants are only allowed inside enum declarations.",
    ),
    (
        "Doppelte Operator?berladung.",
        "Duplicate operator overload.",
    ),
    (
        "Vergleichsoperatoren m?ssen bool zur?ckgeben.",
        "Comparison operators must return bool.",
    ),
    (
        "Konstante '{}' sollte in UPPER_SNAKE_CASE geschrieben werden.",
        "Constant '{}' should be written in UPPER_SNAKE_CASE.",
    ),
    (
        "Nicht jeder Kontrollfluss in '{}' liefert einen R?ckgabewert.",
        "Not every control-flow path in '{}' returns a value.",
    ),
    (
        "Namespace-Segment '{}' sollte in PascalCase geschrieben werden.",
        "Namespace segment '{}' should be written in PascalCase.",
    ),
    (
        "Doppelte Typdeklaration '{}'. Bezeichner sind case-sensitive.",
        "Duplicate type declaration '{}'. Identifiers are case-sensitive.",
    ),
    (
        "Feld '{}' in '{}' hat keinen festen Speicherplatz.",
        "Field '{}' in '{}' has no fixed storage size.",
    ),
    (
        "Struct '{}' enth?lt sich selbst by-value.",
        "Struct '{}' contains itself by value.",
    ),
    (
        "Funktionsdeklaration ben?tigt einen Block oder ';'.",
        "Function declaration requires a block or ';'.",
    ),
    (
        "Konstruktordeklaration ben?tigt einen Block oder ';'.",
        "Constructor declaration requires a block or ';'.",
    ),
    (
        "Operator?berladung ben?tigt einen Block oder ';'.",
        "Operator overload requires a block or ';'.",
    ),
    (
        "Methodendeklaration ben?tigt einen Block oder ';'.",
        "Method declaration requires a block or ';'.",
    ),
];
