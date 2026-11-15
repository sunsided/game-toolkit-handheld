//! A 1-bit dial on the Playdate screen: the crank turns the needle, A inverts the screen.
//!
//! In the simulator, turn the crank with the mouse wheel or Q / E.

use core::convert::Infallible;

use game_toolkit_handheld::{
    Button, HandheldGame, Input,
    embedded_graphics::{
        mono_font::{MonoTextStyle, ascii::FONT_9X15_BOLD},
        pixelcolor::BinaryColor,
        prelude::*,
        primitives::{Circle, Line, PrimitiveStyle},
        text::{Alignment, Text},
    },
};

/// The 1-bit crank dial.
pub struct Dial {
    center: Point,
    radius: i32,
    angle: f32,
    inverted: bool,
}

impl HandheldGame for Dial {
    type Color = BinaryColor;

    fn init(screen: Size) -> Self {
        Self {
            center: Point::new(screen.width as i32 / 2, screen.height as i32 / 2 - 10),
            radius: screen.height as i32 / 3,
            angle: 0.0,
            inverted: false,
        }
    }

    fn update(&mut self, input: &Input, _dt: f32) {
        if let Some(crank) = input.crank() {
            self.angle = crank.angle;
        }
        if input.pressed(Button::A) {
            self.inverted = !self.inverted;
        }
    }

    fn render<D: DrawTarget<Color = BinaryColor, Error = Infallible>>(&mut self, target: &mut D) {
        let (paper, ink) = if self.inverted {
            (BinaryColor::Off, BinaryColor::On)
        } else {
            (BinaryColor::On, BinaryColor::Off)
        };
        let Ok(()) = target.clear(paper);
        let Ok(()) = Circle::with_center(self.center, 2 * self.radius as u32)
            .into_styled(PrimitiveStyle::with_stroke(ink, 3))
            .draw(target);
        // 0 degrees points up, clockwise, like the Playdate crank.
        // micromath: no_std has no float trig; same call on every target.
        let (sin, cos) = micromath::F32Ext::sin_cos(self.angle.to_radians());
        let tip = self.center
            + Point::new(
                (sin * self.radius as f32 * 0.85) as i32,
                (-cos * self.radius as f32 * 0.85) as i32,
            );
        let Ok(()) = Line::new(self.center, tip)
            .into_styled(PrimitiveStyle::with_stroke(ink, 5))
            .draw(target);
        let label = label(self.angle);
        let text = core::str::from_utf8(&label).unwrap_or("");
        let Ok(_) = Text::with_alignment(
            text,
            Point::new(self.center.x, self.center.y + self.radius + 28),
            MonoTextStyle::new(&FONT_9X15_BOLD, ink),
            Alignment::Center,
        )
        .draw(target);
    }
}

/// `"123 deg"` without an allocator, the way a `no_std` board build would format it.
fn label(angle: f32) -> [u8; 7] {
    let deg = angle as u32 % 360;
    let digit = |d: u32| b'0' + d as u8;
    [
        digit(deg / 100),
        digit(deg / 10 % 10),
        digit(deg % 10),
        b' ',
        b'd',
        b'e',
        b'g',
    ]
}
