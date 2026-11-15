//! Kode Dot (ESP32-P4) backend for game-toolkit-handheld: [`run`] drives any `Rgb565`
//! [`HandheldGame`] on the board.
//!
//! The game loop runs in its own std thread on FreeRTOS. Default build: the CO5300 AMOLED over
//! MIPI-DSI, the D-pad and BOT on the TCA6408A expander, TOP on GPIO35, CST820 touch. With the
//! `wokwi` feature: Wokwi's ESP32-P4 with the CO5300, expander and touch controller as Wokwi
//! custom chips; the panel sits on SPI there, since Wokwi has no MIPI-DSI.
//!
//! The app crate needs this crate's ESP-IDF setup: `sdkconfig.defaults`, `.cargo/config.toml`,
//! `rust-toolchain.toml` and `build.rs` (see the README).
//!
//! No sound: the Kode Dot's speaker amp is enabled from the ESP32-C5 co-processor.
#![deny(unsafe_code)]

use std::time::Instant;

use esp_idf_svc::{
    hal::{delay::FreeRtos, peripherals::Peripherals},
    sys::EspError,
};
use game_toolkit_handheld::{
    FrameClock, HandheldGame, Input,
    embedded_graphics::{geometry::Size, pixelcolor::Rgb565},
    screen,
};

mod controls;
#[cfg(not(feature = "wokwi"))]
mod ffi;
#[cfg(not(feature = "wokwi"))]
mod kode;
#[cfg(feature = "wokwi")]
mod wokwi;

#[cfg(not(feature = "wokwi"))]
use kode::Board;
#[cfg(feature = "wokwi")]
use wokwi::Board;

/// The screen the game gets: the 410x502 portrait panel, in Wokwi too.
pub const SCREEN: Size = screen::KODE_DOT;

/// Run `G` on the board: ESP-IDF patches, logging, then the game loop in a 32 KB thread.
///
/// Returns only if the hardware fails; the error is logged.
pub fn run<G: HandheldGame<Color = Rgb565>>() {
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
        .spawn(game_loop::<G>)
        .expect("spawn game thread");
    if let Ok(Err(e)) = game.join() {
        log::error!("game stopped: {e}");
    }
}

/// The game loop: input, update, render, present, forever.
fn game_loop<G: HandheldGame<Color = Rgb565>>() -> Result<(), EspError> {
    let mut board = Board::new(Peripherals::take()?)?;
    let mut game = G::init(SCREEN);
    let mut input = Input::default();
    let mut clock = FrameClock::new();
    let start = Instant::now();
    let (mut frames, mut since) = (0u32, Instant::now());
    log::info!("running at {}x{}", SCREEN.width, SCREEN.height);
    loop {
        input.advance(board.input());
        game.update(&input, clock.tick(start.elapsed().as_micros() as u64));
        board.present(|fb| game.render(fb))?;
        // Block for one tick so this core's idle task runs and feeds the task watchdog.
        FreeRtos::delay_ms(1);

        frames += 1;
        if since.elapsed().as_secs() >= 5 {
            log::info!("{:.1} fps", frames as f32 / since.elapsed().as_secs_f32());
            (frames, since) = (0, Instant::now());
        }
    }
}
