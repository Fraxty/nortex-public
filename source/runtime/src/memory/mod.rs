//! Memory runtime services.

/// Represents a non-owning memory region.
#[derive(Debug, Clone, Copy)]
pub struct MemoryRegion {
    pub start: usize,
    pub length: usize,
}
