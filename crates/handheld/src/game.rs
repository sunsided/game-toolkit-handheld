use core::convert::Infallible;

use embedded_graphics::{draw_target::DrawTarget, geometry::Size, pixelcolor::PixelColor};

use crate::{AudioSource, Input};

/// A game that runs on every backend that supports its [`Color`](HandheldGame::Color).
///
/// ESP32 boards take [`Rgb565`], the Playdate takes [`BinaryColor`], and the simulator takes
/// both. Everything here is called from the backend's main loop or update callback.
///
/// [`Rgb565`]: embedded_graphics::pixelcolor::Rgb565
/// [`BinaryColor`]: embedded_graphics::pixelcolor::BinaryColor
pub trait HandheldGame: Sized + 'static {
    /// The pixel color the game draws with.
    type Color: PixelColor;

    /// Create the game for a screen of `screen` pixels (see [`crate::screen`]).
    fn init(screen: Size) -> Self;

    /// Advance the game by `dt` seconds.
    fn update(&mut self, input: &Input, dt: f32);

    /// Draw the current frame. The target keeps last frame's pixels; clear it if needed.
    fn render<D: DrawTarget<Color = Self::Color, Error = Infallible>>(&mut self, target: &mut D);

    /// The game's sound, called once right after [`init`](HandheldGame::init). `None` (the
    /// default) means silence. The backend moves the source to wherever it mixes audio.
    fn audio(&mut self) -> Option<impl AudioSource> {
        None::<fn(&mut [i16])>
    }
}
