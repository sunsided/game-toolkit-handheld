# game-toolkit-handheld

Write a small game once against a `no_std` framebuffer and input API, then run it on handheld
hardware or, through [game-toolkit](https://github.com/sunsided/game-toolkit), in a desktop or
browser window.

| Target | Screen | Color | Status |
|--------|--------|-------|--------|
| Simulator (game-toolkit, native + web) | any | `Rgb565`, `BinaryColor` | works |
| ESP32-S3 + SPI panel (ILI9341), [`boards/esp32s3`](boards/esp32s3) | 320x240 | `Rgb565` | builds; Embassy + blocking runtimes; Wokwi |
| [Kode Dot](https://kode.diy/) (ESP32-P4), [`boards/kode-dot`](boards/kode-dot) | 410x502 AMOLED (portrait) | `Rgb565` | builds (ESP-IDF 6.1, FreeRTOS); hardware untested; Wokwi stand-in |
| Playdate, `boards/playdate` | 400x240 1-bit | `BinaryColor` | runs in the SDK Simulator; device build untested |

```rust,ignore
use game_toolkit_handheld::{Button, HandheldGame, Input, embedded_graphics::{pixelcolor::BinaryColor, prelude::*}};

struct Game { x: i32 }

impl HandheldGame for Game {
    type Color = BinaryColor;
    fn init(_screen: Size) -> Self { Self { x: 0 } }
    fn update(&mut self, input: &Input, _dt: f32) {
        if input.held(Button::Right) { self.x += 1; }
    }
    fn render<D: DrawTarget<Color = BinaryColor, Error = core::convert::Infallible>>(&mut self, target: &mut D) {
        let Ok(()) = target.clear(BinaryColor::On);
        // draw with embedded-graphics ...
    }
}
```

Run it in the simulator with `sim::run::<Game>(SimConfig::new("game", screen::PLAYDATE))`
(feature `toolkit`). Examples (`crates/demos`, shared with the boards): `cargo run -p game-toolkit-handheld-demos --example maze` (color, sound) and
`--example crank` (1-bit, Playdate crank on mouse wheel or Q / E).

## Runtimes

The core owns no main loop, timer, allocator or interrupt. Each frame a runtime calls
`Input::advance`, `HandheldGame::update` and `HandheldGame::render`, with `FrameClock` turning a
microsecond counter into `dt`. That fits a blocking superloop, an interrupt or RTIC task, an
Embassy `async` task, an RTOS thread (FreeRTOS under ESP-IDF), the Playdate's update callback,
and the game-toolkit simulator alike. Sound is a separate `Send` `AudioSource` the game hands
over once, so an audio interrupt, DMA task or thread owns it while the main loop owns the game.

## Boards

Board crates are standalone (own toolchain each) and stay out of the root workspace; drive them
through the Taskfile: `task --list`, `task doctor` for what is installed.

The Kode Dot backend is a library, `game-toolkit-handheld-kode-dot`, that runs your own game from
your own app crate:

```rust,ignore
fn main() {
    game_toolkit_handheld_kode_dot::run::<MyGame>(); // any HandheldGame<Color = Rgb565>
}
```

Its [README](boards/kode-dot/README.md) lists the ESP-IDF setup the app needs and how to flash
without overwriting kodeOS.

The ESP32-S3 backend is a library too, `game-toolkit-handheld-esp32s3`, for an ESP32-S3 with an
ILI9341 and six buttons on fixed pins. The app owns the runtime (a blocking superloop or
Embassy) and calls `Board::frame` once per frame; its [README](boards/esp32s3/README.md) has the
wiring and the app setup.

The Playdate backend isn't a standalone crate yet and is only used inside this repository. It's
already generic (`run::<G>` plus an entry-point macro), but has no setup docs for outside apps.

Playdate is a registered trademark of Panic. This project is not affiliated with or endorsed by
Panic. Kode Dot is a trademark of Kode DIY.

## License

EUPL-1.2
