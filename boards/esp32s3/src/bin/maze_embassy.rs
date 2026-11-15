//! The maze demo on an Embassy executor, paced by an `embassy_time::Ticker`.
#![no_std]
#![no_main]
#![forbid(unsafe_code)]

use board_esp32s3::Board;
use embassy_executor::Spawner;
use embassy_time::{Duration, Ticker};
use esp_backtrace as _;
use esp_hal::{clock::CpuClock, timer::timg::TimerGroup};
use game_toolkit_handheld::{HandheldGame, screen};
use game_toolkit_handheld_demos::Maze;

esp_bootloader_esp_idf::esp_app_desc!();

/// Frame rate target; one full-frame SPI push takes about 31 ms at 40 MHz, so this is the ceiling.
const FPS: u64 = 30;

#[esp_rtos::main]
async fn main(_spawner: Spawner) -> ! {
    let p = esp_hal::init(esp_hal::Config::default().with_cpu_clock(CpuClock::max()));
    let (mut board, spare) = Board::new(p);
    esp_rtos::start(TimerGroup::new(spare.timg0).timer0, spare.int0);

    let mut game = Maze::init(screen::QVGA);
    esp_println::println!("maze-embassy running");
    let mut ticker = Ticker::every(Duration::from_hz(FPS));
    loop {
        board.frame(&mut game);
        ticker.next().await;
    }
}
