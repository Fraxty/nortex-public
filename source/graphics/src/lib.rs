//! NORTEX graphics workspace crate. The device/backend implementation is not
//! yet available; these modules expose the current architecture skeleton.

#[path = "../backends/mod.rs"]
pub mod backends;
#[path = "../renderer/mod.rs"]
pub mod renderer;
#[path = "../rendergraph/mod.rs"]
pub mod rendergraph;
#[path = "../resources/mod.rs"]
pub mod resources;
#[path = "../rhi/mod.rs"]
pub mod rhi;
