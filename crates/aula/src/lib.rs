//! Driver for the EPOMAKER x AULA F75 Max keyboard. The protocol is described in
//! `docs/protocol.md`, which marks what was checked on hardware.

pub mod descriptor;
pub mod device;
#[cfg(feature = "gif")]
pub mod gif;
pub mod hid;
pub mod layout;
pub mod protocol;

pub use device::Keyboard;
pub use protocol::Connection;
