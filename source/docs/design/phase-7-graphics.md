# NORTEX Phase 7 – Grafiksystem

**Spezifikationsstatus:** Architektur beschlossen  
**Implementierungsstatus:** Module und HLSL-Beispiel vorhanden; echte GPU-Ausführung noch nicht implementiert (unverändert gegenüber dem Entwurf; nicht gegen den Code neu verifiziert)

## 7.1 Schichtenmodell

1. Öffentliche NORTEX Graphics API.
2. Renderer mit Szenen-, Material- und Rendergraph-Verwaltung.
3. Rendering Hardware Interface (RHI).
4. Backend: zuerst Direct3D 12 unter Windows.
5. Spätere Backends: Vulkan; weitere Backends nur bei konkretem Plattformbedarf.

Der Compiler kennt keine GPU- oder Rendering-Semantik. Das Grafiksystem ist eine Runtime-/Bibliothekskomponente und kann unabhängig vom Compiler versioniert werden.

## 7.2 RHI-Vertrag

Das RHI abstrahiert Device, Adapterauswahl, Queue-Typen, Ressourcen, Descriptor-/Bind-Modelle, Command Recording, Synchronisation, Swapchain und Präsentation. Abstraktion darf Backend-Unterschiede nicht verstecken, die für korrekte Synchronisation oder Performance entscheidend sind.

Die API muss Ressourcenbesitz und Lebensdauer nachvollziehbar halten. GPU-Ressourcen können nicht freigegeben werden, solange eingereichte GPU-Arbeit sie noch verwendet. Fence-/Timeline-Werte steuern deshalb die verzögerte Freigabe.

## 7.3 Direct3D-12-Backend

Das erste Backend implementiert:

- DXGI-Adapter- und Device-Auswahl mit Feature-Level-Prüfung.
- Graphics-, Compute- und Copy-Queues.
- Command Allocators und Command Lists mit Pooling.
- Descriptor Heaps und Descriptor-Handle-Verwaltung.
- Buffer, Texturen, Views und Resource States.
- Swapchain, Backbuffer und Render Targets.
- Root Signatures und Pipeline State Objects.
- Shaderkompilierung über DXC und HLSL.
- Fence-Synchronisation, Upload-/Readback-Heaps und GPU-Timing.
- Debug Layer, DRED und verständliche Fehlerdiagnosen.

Ein Gerätverlust wird als eigener Gerätefehler gemeldet; der Renderer darf nicht mit ungültigen Handles weiterarbeiten. Debug Layer und GPU Validation werden in Debug-Builds aktiviert, sofern verfügbar.

## 7.4 Rendergraph

Der Rendergraph beschreibt Render-Pässe, deren Eingabe-/Ausgaberessourcen sowie Abhängigkeiten. Der Graph berechnet eine gültige Ausführungsreihenfolge, Resource Barriers und Lebenszeitintervalle transienter Ressourcen. Zyklen sind Buildfehler des Graphen. Unbenutzte Ressourcen können entfernt werden, sofern sie keine expliziten externen Seiteneffekte besitzen.

## 7.5 Ressourcen und Shader

Ressourcen besitzen eindeutige Beschreibungen: Format, Dimension, Größe, Speicherklasse, Zugriff und Lebensdauer. Shader werden aus HLSL kompiliert; DXC-Version, Zielprofil, Defines und Compilerflags werden im Buildprotokoll festgehalten. Shader-Cache-Schlüssel umfassen den vollständigen Compilerinput und die Toolchain-Version.

## 7.6 Entwicklungsreihenfolge

1. Window-/Surface-Anbindung und Device-Erstellung.
2. Clear-/Triangle-Test mit Swapchain.
3. Buffers, Texturen und Descriptor-Verwaltung.
4. Shader- und Pipeline-Management.
5. Synchronisation und Multi-Frame-Rendering.
6. Rendergraph und Ressourcen-Lifetime.
7. Materialien, Meshes, Kamera und Beleuchtung.
8. Profiling, Debug Layer und Stabilitätstests.

Vulkan wird erst begonnen, wenn die öffentliche RHI-Semantik durch ein produktiv verwendbares D3D12-Backend validiert ist.
