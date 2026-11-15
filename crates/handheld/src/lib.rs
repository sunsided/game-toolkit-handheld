//! Write a small game once against a `no_std` framebuffer and input API, then run it on ESP32
//! boards, the Playdate, or on desktop and web through game-toolkit.
//!
//! A game implements [`HandheldGame`]: it picks its pixel color ([`Rgb565`] for color screens,
//! [`BinaryColor`] for 1-bit ones like the Playdate), reads [`Input`] in `update` and draws into
//! any [`embedded_graphics`] `DrawTarget` in `render`.
//!
//! This crate owns no main loop, timer, allocator or interrupt. Every runtime drives the same
//! three calls: [`Input::advance`], [`HandheldGame::update`] and [`HandheldGame::render`], with
//! [`FrameClock`] turning a microsecond counter into `dt`. That works the same from a blocking
//! superloop, an interrupt or RTIC task, an Embassy `async` task, an RTOS thread, the Playdate's
//! update callback, or the game-toolkit simulator (`toolkit` feature, [`sim`]). Sound is a
//! separate [`AudioSource`] handed over once, so an audio interrupt, DMA task or thread can own
//! it while the main loop owns the game.
//!
//! [`Rgb565`]: embedded_graphics::pixelcolor::Rgb565
//! [`BinaryColor`]: embedded_graphics::pixelcolor::BinaryColor
#![cfg_attr(not(feature = "toolkit"), no_std)]
#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]

mod audio;
mod clock;
mod framebuffer;
mod game;
mod input;
pub mod screen;
#[cfg(feature = "toolkit")]
pub mod sim;

pub use audio::{AUDIO_RATE, AudioSource};
pub use clock::{FrameClock, MAX_DT};
pub use embedded_graphics;
pub use framebuffer::{Framebuffer1, Framebuffer565};
pub use game::HandheldGame;
pub use input::{Button, Buttons, Crank, Input, InputState, Motion};
