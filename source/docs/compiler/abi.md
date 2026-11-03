# NORTEX ABI

Der verbindliche Zielvertrag für Windows x64, Struct-Layout, C-ABI, Symbolnamen, Speicherbesitz, Exceptions und ABI-Versionierung steht in [Phase 4](../design/phase-4-native-runtime-abi.md).

Für Windows x64 wird die Microsoft x64 Calling Convention verwendet. `extern "C"` markiert Funktionen mit C-ABI. Nur ausdrücklich ABI-kompatible Datentypen dürfen die Grenze passieren. `string`, Klassen-Owner, `Task<T>`, `Future<T>` und NORTEX-Exceptions sind nicht direkt C-ABI-kompatibel. Strings werden über UTF-8-Zeiger plus Byte-Länge transportiert; Allokation und Freigabe müssen demselben vereinbarten Allocator-Vertrag folgen.

**Status:** ABI-Spezifikation vorhanden, aber noch keine stabile native ABI implementiert oder freigegeben. Vor einer Verwendung für Binärkompatibilität sind FFI-Tests und ein ABI-Freeze erforderlich.
