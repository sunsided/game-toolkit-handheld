# game-toolkit-handheld

Write a small game once against a `no_std` framebuffer and input API, then run it on handheld
hardware or, through [game-toolkit](https://github.com/sunsided/game-toolkit), in a desktop or
browser window.

| Target | Screen | Color | Status |
|--------|--------|-------|--------|
| Simulator (game-toolkit, native + web) | any | `Rgb565`, `BinaryColor` | works |
| ESP32-S3 + SPI panel (ST7789 / ILI9341) | 320x240 | `Rgb565` | planned |
| [Kode Dot](https://kode.diy/) (ESP32-P4) | 502x410 AMOLED | `Rgb565` | planned |
| Playdate | 400x240 1-bit | `BinaryColor` | planned |

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
(feature `toolkit`). Examples: `cargo run --all-features --example maze` (color, sound) and
`--example crank` (1-bit, Playdate crank on mouse wheel or Q / E).

## Runtimes

The core owns no main loop, timer, allocator or interrupt. Each frame a runtime calls
`Input::advance`, `HandheldGame::update` and `HandheldGame::render`, with `FrameClock` turning a
microsecond counter into `dt`. That fits a blocking superloop, an interrupt or RTIC task, an
Embassy `async` task, an RTOS thread (FreeRTOS under ESP-IDF), the Playdate's update callback,
and the game-toolkit simulator alike. Sound is a separate `Send` `AudioSource` the game hands
over once, so an audio interrupt, DMA task or thread owns it while the main loop owns the game.

## License

EUPL-1.2
