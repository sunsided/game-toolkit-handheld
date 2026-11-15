use embedded_graphics::geometry::Point;

/// A button every supported device has, or that the backend maps onto one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Button {
    /// D-pad up.
    Up,
    /// D-pad down.
    Down,
    /// D-pad left.
    Left,
    /// D-pad right.
    Right,
    /// Primary action.
    A,
    /// Secondary action.
    B,
    /// Menu / start. The Playdate keeps its menu button for the system, so it never reports it.
    Menu,
}

impl Button {
    const fn bit(self) -> u8 {
        1 << self as u8
    }
}

/// A set of [`Button`]s, e.g. the ones held down right now.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Buttons(u8);

impl Buttons {
    /// No button.
    pub const NONE: Self = Self(0);

    /// This set plus `button`.
    #[must_use]
    pub const fn with(self, button: Button) -> Self {
        Self(self.0 | button.bit())
    }

    /// Add `button` if `down`, remove it otherwise.
    pub fn set(&mut self, button: Button, down: bool) {
        if down {
            self.0 |= button.bit();
        } else {
            self.0 &= !button.bit();
        }
    }

    /// Whether `button` is in the set.
    pub const fn contains(self, button: Button) -> bool {
        self.0 & button.bit() != 0
    }
}

/// Accelerometer and, where present, gyroscope and magnetometer readings.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Motion {
    /// Acceleration in g, device axes (x right, y down the screen, z out of the screen).
    pub accel: [f32; 3],
    /// Angular rate in degrees per second, same axes.
    pub gyro: Option<[f32; 3]>,
    /// Magnetic field in microtesla, same axes.
    pub mag: Option<[f32; 3]>,
}

/// The Playdate crank, or whatever the backend maps onto one (the simulator: mouse wheel, Q/E).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Crank {
    /// Absolute angle in degrees, `0.0..360.0`, clockwise.
    pub angle: f32,
    /// Degrees turned since the previous frame, positive clockwise.
    pub delta: f32,
}

/// What the backend read from the hardware this frame; fed to [`Input::advance`].
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct InputState {
    /// Buttons held down right now.
    pub buttons: Buttons,
    /// Touch point in screen pixels, `None` when nothing touches or there is no touch screen.
    pub touch: Option<Point>,
    /// Motion sensors, `None` without any.
    pub motion: Option<Motion>,
    /// Crank, `None` when docked or absent.
    pub crank: Option<Crank>,
}

/// Input as the game sees it: this frame's [`InputState`] plus the button edges since the
/// previous frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Input {
    now: InputState,
    previous: Buttons,
}

impl Input {
    /// Start the next frame with `state`. Call once per [`update`](crate::HandheldGame::update).
    pub fn advance(&mut self, state: InputState) {
        self.previous = self.now.buttons;
        self.now = state;
    }

    /// `button` is down.
    pub fn held(&self, button: Button) -> bool {
        self.now.buttons.contains(button)
    }

    /// `button` went down this frame.
    pub fn pressed(&self, button: Button) -> bool {
        self.held(button) && !self.previous.contains(button)
    }

    /// `button` went up this frame.
    pub fn released(&self, button: Button) -> bool {
        !self.held(button) && self.previous.contains(button)
    }

    /// Touch point in screen pixels.
    pub fn touch(&self) -> Option<Point> {
        self.now.touch
    }

    /// Motion sensor readings.
    pub fn motion(&self) -> Option<Motion> {
        self.now.motion
    }

    /// The crank.
    pub fn crank(&self) -> Option<Crank> {
        self.now.crank
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(input: &mut Input, buttons: Buttons) {
        input.advance(InputState {
            buttons,
            ..InputState::default()
        });
    }

    #[test]
    fn edges_last_one_frame() {
        let mut input = Input::default();
        let a = Buttons::NONE.with(Button::A);

        frame(&mut input, a);
        assert!(input.pressed(Button::A) && input.held(Button::A));
        assert!(!input.pressed(Button::B));

        frame(&mut input, a);
        assert!(!input.pressed(Button::A) && input.held(Button::A));

        frame(&mut input, Buttons::NONE);
        assert!(input.released(Button::A) && !input.held(Button::A));

        frame(&mut input, Buttons::NONE);
        assert!(!input.released(Button::A));
    }

    #[test]
    fn buttons_set_and_clear() {
        let mut b = Buttons::NONE;
        b.set(Button::Menu, true);
        b.set(Button::Up, true);
        b.set(Button::Up, false);
        assert!(b.contains(Button::Menu) && !b.contains(Button::Up));
    }
}
