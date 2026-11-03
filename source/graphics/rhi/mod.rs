//! Rendering Hardware Interface.

pub trait GraphicsDevice {
    fn backend_name(&self) -> &'static str;
}
