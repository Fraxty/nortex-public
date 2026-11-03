//! Allocation runtime services.

/// Allocation statistics placeholder.
#[derive(Debug, Default)]
pub struct AllocationStats {
    pub allocations: usize,
    pub bytes_allocated: usize,
}
