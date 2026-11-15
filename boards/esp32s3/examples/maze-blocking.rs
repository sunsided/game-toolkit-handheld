//! The maze demo in a plain superloop; the SPI push paces it.
#![no_std]
#![no_main]
#![forbid(unsafe_code)]

use esp_backtrace as _;
use esp_hal::{clock::CpuClock, main};
use game_toolkit_handheld::HandheldGame;
use game_toolkit_handheld_demos::Maze;
use game_toolkit_handheld_esp32s3::{Board, SCREEN};

esp_bootloader_esp_idf::esp_app_desc!();

#[main]
fn main() -> ! {
    let p = esp_hal::init(esp_hal::Config::default().with_cpu_clock(CpuClock::max()));
    let (mut board, _) = Board::new(p);
    let mut game = Maze::init(SCREEN);
    esp_println::println!("maze-blocking running");
    loop {
        board.frame(&mut game);
    }
}
