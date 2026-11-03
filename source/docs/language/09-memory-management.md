# Kapitel 9 – Speicherverwaltung

← [Kapitel 8](08-structs-and-data-structures.md) · [Inhalt](README.md) · Weiter: [Kapitel 10](10-input-output.md)

## Aktuelles natives Speichermodell

Skalare Locals und Parameter liegen in Registern/Stack-Slots. Klasseninstanzen liegen im Heap und werden als Zeigerwerte gespeichert. Zuweisung, Parameterübergabe und Rückgabe kopieren den Zeiger, nicht das Objekt; mehrere Bindungen können daher dieselbe Instanz verändern. String-Felder enthalten den bestehenden NORTEX-String-Slice-Deskriptor.

| Bereich | Verhalten |
|---|---|
| Objektallokation | `new Class(...)` ruft `nortex_object_alloc` auf; Speicher ist nullinitialisiert und 8-Byte-ausgerichtet. |
| Objektlebensdauer | Die Arena hält jede Instanz bis zur normalen Rückkehr aus dem generierten `main`; dort werden alle Allokationen gemeinsam freigegeben. |
| Zyklen | Referenzzyklen sind gültig und erfordern keine Referenzzählung. |
| Freigabe / GC | Keine Einzelobjektfreigabe und kein Tracing-GC. Speicherverbrauch wächst bis zur Rückkehr aus `main` und kann während langer Programmläufe unbeschränkt wachsen. |
| Null-Referenzen | Es gibt kein Nullliteral. Noch nicht gesetzte Klassenreferenzfelder starten intern als Null; ein Zugriff auf ein Nullobjekt trappt statt Speicher zu dereferenzieren. Initialisiere Referenzfelder vor dem Zugriff. |

Die Runtime besitzt die Allokationen in einer synchronisierten Arena und gibt sie bei normaler Rückkehr aus generiertem `main` frei. Ein Trap oder Prozessabbruch umgeht diesen expliziten Shutdown; dann räumt das Betriebssystem beim Prozessende auf. Wird ein Programm ohne generierten `main` eingebettet, muss der Aufrufer `nortex_object_arena_shutdown` erst aufrufen, wenn keine Objektzeiger mehr verwendet werden. Laufzeitige Einzelobjektfreigabe und Tracing-GC gibt es nicht: Der Compiler emittiert keine Stack-Maps oder Root-Registrierung, daher kann mark-and-sweep keine lebenden Objekte sicher ermitteln. Bei Nebenläufigkeit müssen alle Threads, die Objektzeiger nutzen, vor dem Ende von `main` beendet sein. Es gibt keine Destruktoren oder deterministische Ressourcenfreigabe.

## Weitere Grenzen

- `struct`, `record`, Arrays, `Box<T>`, `Rc<T>` und `Arc<T>` sind im nativen Backend noch nicht implementiert.
- Geschlossene generische Klassen sowie generische Instanzmethoden mit expliziten konkreten Typargumenten, Einzelvererbung und `virtual`/`override`-Dispatch werden unterstützt. Basiskonstruktoren müssen parameterlos sein; generische freie Funktionen, implizite Methoden-Typargument-Inferenz, Bounds, Mehrfachvererbung und Interfaces/Traits fehlen; siehe [Kapitel 8](08-structs-and-data-structures.md).
- Raw Pointer und `unsafe` sind keine unterstützten Quellsprachenfeatures.
- Skalare sind Copy. Klassenreferenzen sind aliasierbare Zeiger; vollständige Ownership-/Borrow-/Lifetime-Prüfung gilt für diese Referenzen noch nicht.

Die Runtime-Arena wird bei nativen Programmen automatisch eingebunden, wenn der Compiler Objektallokationen emittiert. Siehe auch [Compiler-Codegenerierung](../compiler/codegen.md).
