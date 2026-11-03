# NORTEX – Ownership und Borrowing 0.3.7-dev

NORTEX verwendet keinen Garbage Collector. Jeder besitzende Wert hat einen eindeutigen Eigentümer. Die Regeln werden zur Compile-Zeit durch Ownership- und Borrow-Analyse geprüft.

## Verbindliche Regeln

1. Ein besitzender Wert besitzt genau einen Owner.
2. Übergabe oder Zuweisung nach Wert überträgt die Eigentümerschaft (Move), sofern der Typ nicht ausdrücklich `Copy` implementiert.
3. Nach einem Move ist die alte Bindung nicht mehr verwendbar, bis sie vollständig neu initialisiert wurde.
4. Beliebig viele gleichzeitige unveränderliche Borrows (`&T`) sind zulässig.
5. Ein veränderlicher Borrow (`&mut T`) ist exklusiv; währenddessen bestehen keine weiteren aktiven Borrows auf denselben Speicherbereich.
6. Ein Borrow darf nicht länger leben als der referenzierte Wert.
7. `clone()` ist ausdrücklich und typspezifisch. Es gibt keine implizite tiefe Kopie.
8. Gemeinsames Eigentum wird ausdrücklich über `Rc<T>` (ein Thread) oder `Arc<T>` (threadsicher) gewählt.
9. `Span<T>` ist eine nicht besitzende, lebenszeitgebundene Sicht. `Memory<T>` ist ein besitzender Runtime-Handle.
10. Destruktoren laufen deterministisch beim Ende der Lebensdauer und während Exception-Unwinding; sie dürfen keine Exceptions auslösen.

## Syntax

```nortex
public void displayPlayer(&Player player)
{
    print(player.health);
}

public void damagePlayer(&mut Player player)
{
    player.health = player.health - 10;
}

Player duplicate = player.clone();
```

## Klassen und Heap

Eine mit `new` erzeugte Klasseninstanz besitzt einen eindeutigen Heap-Owner. Das Variablenbinding hält diesen Owner und ein Move überträgt ihn. `Box<T>` ist ein gesonderter eindeutiger Heap-Owner für beliebige Werte und explizite Indirektion. `Rc<T>`/`Arc<T>` erlauben geteiltes Eigentum nur mit ihren jeweiligen Synchronisationsgarantien.

## Unsafe

Raw Pointer, manuelle Speicherverwaltung, Pointer-Arithmetik und bestimmte FFI-Vorgänge benötigen einen abgegrenzten `unsafe { ... }`-Block. Der Block dokumentiert die Verantwortung des Autors für Gültigkeit, Alignment, Alias-Regeln und Lebensdauer. `unsafe` deaktiviert nicht die Typprüfung des übrigen Programms.

## Implementierungsstand

Der Compiler prüft derzeit Moves aus lokalen Bindungen bei By-Value-Aufrufen,
Variableninitialisierungen, Rückgaben und Zuweisungen. Eine vollständig
neu zugewiesene `mut`-Bindung ist anschließend wieder verwendbar. Primitive
Skalare werden als kopierbar behandelt; andere Werte werden konservativ als
besitzend behandelt. `&T`-Parameter verhindern den Move; mehrfaches Borrowing
innerhalb eines Aufrufs wird auf Konflikte mit `&mut T` geprüft. Ein `&mut`
Borrow aus einer unveränderlichen Bindung wird abgelehnt. Move-Zustände sind
an einzelne lokale Bindungen gebunden, nicht nur an deren Namen; gleichnamige
Variablen in getrennten lexikalischen Scopes beeinflussen sich daher nicht.
Bei Borrow-Argumenten desselben Aufrufs werden bekannte, verschiedene
Feldprojektionen als disjunkt erkannt; unbekannte Wurzeln bleiben konservativ
aliasierend.

Der Rückgabepfad-Checker betrachtet Sequenzen, Verzweigungen, Switches, Loops,
Match-Arme, Throws und `finally`. Exhaustive Enum-Matches über Enum-Parameter
werden als vollständige Kontrollflussverzweigung erkannt. `Result<T,E>`-
Aufrufresultate dürfen nicht als eigenständige Ausdrucksstatements verworfen
werden.

Die MIR-Pipeline verfolgt lokale Loans durch Referenzkopien, Reborrows,
Kontrollfluss und lexikalische Storage-Regionen. Dereferenzierte Schreibzugriffe
werden auf den geliehenen Speicherbereich zurückgeführt; Elternreferenzen sind
während aktiver Reborrows gesperrt. MIR-Feldprojektionen unterstützen disjunkte
Loans, partielle Moves und die erneute Initialisierung bewegter Felder, sofern
die Feldtypen im MIR deklariert sind. Referenzargumente eines MIR-Calls werden
auf überlappende shared/mutable Loans geprüft. Referenzrückgaben aus
Referenzparametern sind zulässig; Referenzresultate unbekannter Calls werden
wegen fehlender Lifetime-Metadaten abgewiesen. Die Negativfixtures liegen in
`compiler/tests/negatives/`.

Das ist weiterhin kein vollständiger Ownership-/Borrow-Checker:
Loan-Enden sind lexikalisch statt NLL, explizite Lifetime-Parameter und
regionsbasierte Inferenz fehlen. Der Frontend-Lowerer bildet Funktionsaufrufe
und Aggregate noch nicht vollständig auf MIR ab; Feldprojektionen sind daher
bislang MIR-seitig und nicht als vollständige Quellsprachfunktion verfügbar.
`Copy`-Trait-Regeln und Drop-Reihenfolge sind ebenfalls unvollständig. Der
aktuelle Stand darf nicht als vollständige Lifetime-Sicherheit ausgegeben
werden.

Ein vollständiger Borrow Checker benötigt zusätzlich Control-Flow-Analyse,
Loan-Tracking, Lifetime-Inferenz/-Prüfung, Feld- und Teilfeld-Loans über
Aufrufgrenzen hinweg, Reborrow-Regeln, Pinning-Verträge und korrekte
Drop-Reihenfolge.
