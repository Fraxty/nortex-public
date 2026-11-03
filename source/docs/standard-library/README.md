# NORTEX-Standardbibliothek

Die Modul- und API-Regeln sind in [Phase 6](../design/phase-6-standard-library.md) festgehalten. Vorgesehene Namespaces sind `System`, `System.Collections`, `System.IO`, `System.Net`, `System.Threading`, `System.Async`, `System.Text`, `System.Math`, `System.Time`, `System.Diagnostics`, `System.Process`, `System.Memory` und `System.Security`.

`List<T>` und `Vec<T>` sind unterschiedliche nominale Typen. Beide sind dynamisch und dürfen intern auf zusammenhängenden Puffern beruhen; ihre APIs und Typidentität bleiben getrennt. `Option<T>` und `Result<T,E>` repräsentieren optionale Werte beziehungsweise erwartbare Fehler. Sammlungsindizes und Längen verwenden `usize`; `get` liefert `Option<&T>`, verändernder Zugriff `Option<&mut T>`.

**Implementierungsstatus:** Die vorhandenen `.nx`-Dateien sind API-Entwürfe und noch keine vollständige Implementierung. Speicherverwaltung, Laufzeitfunktionen, Tests, vollständige Modulauflösung und Paketauflösung fehlen.
