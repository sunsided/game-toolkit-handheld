# game-toolkit-handheld-esp32s3

Runs a [game-toolkit-handheld](../../crates/handheld) game on an ESP32-S3 with an ILI9341
320x240 SPI panel and six buttons, on bare-metal [esp-hal](https://crates.io/crates/esp-hal)
(`no_std`, no RTOS).

The wiring is fixed. Build the board to match it:

| Signal | GPIO | | Button | GPIO |
|--------|------|-|--------|------|
| SCK    | 12   | | Up     | 4    |
| MOSI   | 11   | | Down   | 5    |
| CS     | 10   | | Left   | 6    |
| DC     | 9    | | Right  | 7    |
| RST    | 8    | | A      | 15   |
|        |      | | B      | 16   |

Buttons connect to GND when pressed; the board enables internal pull-ups. The panel's
backlight (LED) goes to 3V3. `diagram.json` wires the same pins in Wokwi.

## Use

The app owns the runtime and calls `Board::frame` once per frame: input, update, render and
the SPI push. A blocking superloop:

```rust
#![no_std]
#![no_main]

use esp_backtrace as _;
use esp_hal::{clock::CpuClock, main};
use game_toolkit_handheld::HandheldGame;
use game_toolkit_handheld_esp32s3::{Board, SCREEN};

esp_bootloader_esp_idf::esp_app_desc!();

#[main]
fn main() -> ! {
    let p = esp_hal::init(esp_hal::Config::default().with_cpu_clock(CpuClock::max()));
    let (mut board, _spare) = Board::new(p);
    let mut game = MyGame::init(SCREEN); // any HandheldGame<Color = Rgb565>
    loop {
        board.frame(&mut game);
    }
}
```

For Embassy, hand `Spare`'s timer group and software interrupt to `esp_rtos::start` and pace
`frame` with a `Ticker`. One full-frame push takes about 31 ms at 40 MHz. The `maze-blocking`
and `maze-embassy` examples show both runtimes.

## App setup

Install the Xtensa toolchain with [espup](https://github.com/esp-rs/espup)
(`espup install --targets esp32s3`) and source `~/export-esp.sh` before building. Copy these
files from this directory into your app:

- `.cargo/config.toml`: the `xtensa-esp32s3-none-elf` target, `build-std = ["core"]`, the
  linker flags, and `espflash` as the runner.
- `rust-toolchain.toml`: the `esp` channel.
- `build.rs`: links esp-hal's `linkall.x`. Linker arguments don't carry over from a
  dependency, so the app needs its own.

Your app's `Cargo.toml` takes the library plus what the example above uses, at the versions
this crate's `Cargo.toml` lists, so they match its esp-hal:

```toml
[dependencies]
game-toolkit-handheld = "0.1"
game-toolkit-handheld-esp32s3 = "0.1"
esp-hal = { version = "~1.2.2", features = ["esp32s3", "unstable"] }
esp-bootloader-esp-idf = { version = "0.6.0", features = ["esp32s3"] }
esp-backtrace = { version = "0.20.0", features = ["esp32s3", "panic-handler", "println"] }
esp-println = { version = "0.18.0", features = ["esp32s3"] }
```

`cargo run --release` then flashes and opens the serial monitor.

In this repository: `task s3:build`, `task s3:flash` (`BIN=maze-blocking` for the superloop),
and `task s3:wokwi` / `task s3:wokwi:blocking` for Wokwi.
