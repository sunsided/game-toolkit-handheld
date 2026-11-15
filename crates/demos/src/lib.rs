//! Demo games shared by the simulator examples and every board crate, so each backend runs
//! the same code.
//!
//! - [`Maze`]: `Rgb565`, D-pad, sound (ESP32 boards, simulator).
//! - [`Dial`]: `BinaryColor`, crank, A button (Playdate, simulator).
#![no_std]
#![forbid(unsafe_code)]

mod dial;
mod maze;

pub use dial::Dial;
pub use maze::Maze;
