//! Kode Dot (ESP32-P4) backend for game-toolkit-handheld, running the `Maze` demo.
//!
//! The game loop runs in its own std thread on FreeRTOS. Default build: the CO5300 AMOLED over
//! MIPI-DSI, the D-pad and BOT on the TCA6408A expander, TOP on GPIO35, CST820 touch. With the
//! `wokwi` feature: an ILI9341 SPI panel and plain GPIO buttons in Wokwi's ESP32-P4 instead.
//!
//! No sound: the Kode Dot's speaker amp is enabled from the ESP32-C5 co-processor.
#![deny(unsafe_code)]

use std::time::Instant;

use esp_idf_svc::{hal::peripherals::Peripherals, sys::EspError};
use game_toolkit_handheld::{FrameClock, HandheldGame, Input};
use game_toolkit_handheld_demos::Maze;

#[cfg(not(feature = "wokwi"))]
mod ffi;
#[cfg(not(feature = "wokwi"))]
mod kode;
#[cfg(feature = "wokwi")]
mod wokwi;

#[cfg(not(feature = "wokwi"))]
use kode::{Board, SCREEN};
#[cfg(feature = "wokwi")]
use wokwi::{Board, SCREEN};

fn main() {
    esp_idf_svc::sys::link_patches();
    let logger = esp_idf_svc::log::init_from_esp_idf();
    // The CST820 NAKs while it dozes; the I2C driver would log that every frame.
    #[cfg(not(feature = "wokwi"))]
    if let Err(e) = logger
        .filter()
        .set_target_level("i2c", log::LevelFilter::Off)
    {
        log::warn!("can't silence i2c logs: {e}");
    }
    #[cfg(feature = "wokwi")]
    let _ = logger;

    let game = std::thread::Builder::new()
        .name("game".into())
        .stack_size(32 * 1024)
        .spawn(run)
        .expect("spawn game thread");
    if let Ok(Err(e)) = game.join() {
        log::error!("game stopped: {e}");
    }
}

/// The game loop: input, update, render, present, forever.
fn run() -> Result<(), EspError> {
    let mut board = Board::new(Peripherals::take()?)?;
    let mut game = Maze::init(SCREEN);
    let mut input = Input::default();
    let mut clock = FrameClock::new();
    let start = Instant::now();
    let (mut frames, mut since) = (0u32, Instant::now());
    log::info!("running Maze at {}x{}", SCREEN.width, SCREEN.height);
    loop {
        input.advance(board.input());
        game.update(&input, clock.tick(start.elapsed().as_micros() as u64));
        board.present(|fb| game.render(fb))?;

        frames += 1;
        if since.elapsed().as_secs() >= 5 {
            log::info!("{:.1} fps", frames as f32 / since.elapsed().as_secs_f32());
            (frames, since) = (0, Instant::now());
        }
    }
}
