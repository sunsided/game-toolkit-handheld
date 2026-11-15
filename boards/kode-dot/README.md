# game-toolkit-handheld-kode-dot

Runs a [game-toolkit-handheld](https://crates.io/crates/game-toolkit-handheld) game on the
[Kode Dot](https://kode.diy/) (ESP32-P4): the 410x502 CO5300 AMOLED over MIPI-DSI, the D-pad and
BOT through the TCA6408A expander, TOP on GPIO35, and CST820 touch.

> **Alpha:** this has run in Wokwi but not yet on a real Kode Dot.

```rust
use game_toolkit_handheld_kode_dot::run;

fn main() {
    run::<MyGame>(); // any HandheldGame<Color = Rgb565>
}
```

`run` applies the ESP-IDF patches, sets up logging and runs the game loop in its own thread.
It returns only when the hardware fails. There's no sound yet: the speaker amp is enabled from
the ESP32-C5 co-processor.

## App setup

The crate builds on ESP-IDF 6.1.1 through `esp-idf-sys`, and ESP-IDF reads part of its
configuration from the app crate. Copy these files from this directory into your app:

- `sdkconfig.defaults`: PSRAM, the 32 MB flash, `LCD_DSI_ISR_CACHE_SAFE` (the panel needs it),
  `FREERTOS_HZ=1000` and the bigger main and pthread stacks.
- `.cargo/config.toml`: the `riscv32imafc-esp-espidf` target, `ldproxy`, `build-std` and
  `ESP_IDF_VERSION`.
- `rust-toolchain.toml`: the pinned nightly. Newer std doesn't build for espidf.
- `build.rs`, with `embuild` as a build dependency.

Your app's `Cargo.toml` needs two more entries:

```toml
[dependencies]
game-toolkit-handheld-kode-dot = "0.1"
# build.rs reads the ESP-IDF link args from this; it must be a direct dependency.
esp-idf-svc = "0.53"

[build-dependencies]
embuild = "0.33"
```

The CO5300 panel driver (`espressif/esp_lcd_co5300`) comes with this crate. esp-idf-sys picks
up extra components from direct dependencies, so your app should depend on this crate directly.

Install the tools once: `cargo install ldproxy espflash --locked`.

## Flashing

kodeOS owns the bootloader. Don't flash at `0x0`: `espflash flash` would replace kodeOS.
Instead, write the app image into kodeOS's `app_pending` mailbox, and kodeOS adopts it on the
next boot:

```sh
espflash save-image --chip esp32p4 target/riscv32imafc-esp-espidf/release/my-game my-game.bin
espflash write-bin 0x1A00000 my-game.bin
```

## Wokwi

The `wokwi` feature runs the board in Wokwi's ESP32-P4. The CO5300, TCA6408A and CST820 run as
Wokwi custom chips from their driver crates, so the game sees the same 410x502 screen and input
goes through the same code as on hardware. Only the panel transport differs: Wokwi has no
MIPI-DSI, so the CO5300 sits on SPI and the `co5300` crate drives it. Wokwi's P4 model doesn't
handle PSRAM, so layer `sdkconfig.wokwi` over the defaults with
`ESP_IDF_SDKCONFIG_DEFAULTS="sdkconfig.defaults;sdkconfig.wokwi"`.
`diagram.json` and `wokwi.toml` show the wiring. In this repository, `task kode:wokwi` runs the
`maze` example there.
