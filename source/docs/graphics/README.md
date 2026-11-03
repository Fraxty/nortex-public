# NORTEX Graphics

Die Architektur ist in [Phase 7](../design/phase-7-graphics.md) festgelegt.

Schichten: öffentliche Graphics API → Renderer/Rendergraph → Rendering Hardware Interface (RHI) → Direct3D-12-Backend. Vulkan ist als späteres Backend vorgesehen. HLSL wird über DXC kompiliert. Compiler und Grafiksystem bleiben getrennte Subsysteme.

Das D3D12-Backend muss Device-/Adapterauswahl, Queues, Command Lists, Descriptor Heaps, Ressourcen, Swapchain, Render Targets, PSOs, Fence-Synchronisation und Fehlerdiagnose bereitstellen. Der Rendergraph verwaltet Abhängigkeiten, Barriers und Lebenszeiten transienter Ressourcen.

**Implementierungsstatus:** Grundmodule, einzelne Ressourcenbeschreibungen und HLSL-Beispiele sind vorhanden. Geräteerstellung, Swapchain, echte Ressourcenverwaltung, Command Recording und Rendergraph-Ausführung sind nicht implementiert.
