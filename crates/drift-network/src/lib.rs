//! Unprivileged local data plane. Only explicitly proxied sockets enter here.
pub mod apps;
pub mod broker;
pub mod dns;
pub mod pac;
pub mod platform;
pub mod socks;

pub mod router;
