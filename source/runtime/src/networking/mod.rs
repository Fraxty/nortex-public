//! Networking runtime services.

#[derive(Debug, Clone)]
pub struct Endpoint {
    pub host: String,
    pub port: u16,
}
