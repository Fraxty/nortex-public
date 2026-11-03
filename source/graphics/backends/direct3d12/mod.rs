//! Direct3D 12 backend boundary. Device creation and command submission are pending.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direct3D12Error {
    NotImplemented,
}

/// Placeholder marker for the planned Windows D3D12 device implementation.
#[derive(Debug, Default)]
pub struct Direct3D12Device;
