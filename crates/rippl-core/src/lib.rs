//! Everything rippl decides that does not need a window or an engine to
//! decide. Kept free of GTK and WebKit so all of it runs under `cargo test`
//! with no display.

pub mod address;
pub mod downloads;
pub mod places;
pub mod session;
pub mod shield;
pub mod suggest;
pub mod tabs;
