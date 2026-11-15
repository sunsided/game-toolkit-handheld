//! The real Kode Dot: power-up, CO5300 panel, expander buttons, TOP button and CST820 touch.

use std::{thread::sleep, time::Duration};

use esp_idf_svc::{
    hal::{
        delay::TickType,
        gpio::{self, PinDriver, Pull},
        i2c::{I2cConfig, I2cDriver},
        ldo::{Adjustable, LdoChannel, config::LdoChannelConfig},
        peripherals::Peripherals,
        units::FromValueType,
    },
    sys::EspError,
};
use game_toolkit_handheld::{
    Button, Buttons, Framebuffer565, InputState,
    embedded_graphics::{geometry::Size, prelude::Point},
    screen,
};

use crate::ffi::Panel;

/// The game sees the whole 410x502 portrait panel.
pub const SCREEN: Size = screen::KODE_DOT;

/// TCA6408A I/O expander.
const EXPANDER: u8 = 0x20;
const EXP_INPUT: u8 = 0x00;
const EXP_OUTPUT: u8 = 0x01;
const EXP_CONFIG: u8 = 0x03;
/// P7 drives VCI_EN, the panel supply; P0..P6 are inputs.
const VCI_EN: u8 = 1 << 7;
/// Expander inputs, active low. Holding BOT is kodeOS's home gesture, so B never needs a hold.
const PAD: [(u8, Button); 5] = [
    (0, Button::B),
    (1, Button::Up),
    (2, Button::Left),
    (3, Button::Down),
    (4, Button::Right),
];
/// CST820 touch controller.
const TOUCH: u8 = 0x15;
/// A stuck bus costs a frame, not the game.
const I2C_TIMEOUT: TickType = TickType::new_millis(10);

/// Everything the game loop needs from the hardware.
pub struct Board {
    i2c: I2cDriver<'static>,
    top: PinDriver<'static, gpio::Input>,
    /// Held high: dropping it would float the display + touch reset line.
    _reset: PinDriver<'static, gpio::Output>,
    /// Keeps the DSI PHY powered.
    _phy_ldo: LdoChannel<'static, Adjustable>,
    panel: Panel,
}

impl Board {
    /// Power up in the order the board needs: panel supply, reset, PHY LDO, then DSI.
    pub fn new(p: Peripherals) -> Result<Self, EspError> {
        let mut i2c = I2cDriver::new(
            p.i2c0,
            p.pins.gpio4,
            p.pins.gpio5,
            &I2cConfig::new().baudrate(400.kHz().into()),
        )?;
        // Latch VCI_EN high before P7 turns into an output, so it never glitches low.
        i2c.write(EXPANDER, &[EXP_OUTPUT, VCI_EN], I2C_TIMEOUT.ticks())?;
        i2c.write(EXPANDER, &[EXP_CONFIG, !VCI_EN], I2C_TIMEOUT.ticks())?;
        sleep(Duration::from_millis(20));

        let mut reset = PinDriver::output(p.pins.gpio7)?;
        reset.set_low()?;
        sleep(Duration::from_millis(10));
        reset.set_high()?;
        sleep(Duration::from_millis(120));

        let phy_ldo = LdoChannel::new(p.ldo3, &LdoChannelConfig::new(2500))?;
        let panel = Panel::new()?;

        Ok(Self {
            i2c,
            top: PinDriver::input(p.pins.gpio35, Pull::Up)?,
            _reset: reset,
            _phy_ldo: phy_ldo,
            panel,
        })
    }

    /// Buttons and touch, read now.
    pub fn input(&mut self) -> InputState {
        let mut buttons = Buttons::NONE;
        let mut levels = [0xFF];
        if self
            .i2c
            .write_read(EXPANDER, &[EXP_INPUT], &mut levels, I2C_TIMEOUT.ticks())
            .is_ok()
        {
            for (bit, button) in PAD {
                buttons.set(button, levels[0] & (1 << bit) == 0);
            }
        }
        buttons.set(Button::A, self.top.is_low());
        InputState {
            buttons,
            touch: self.touch(),
            ..InputState::default()
        }
    }

    /// One CST820 point: gesture, finger count, then 12-bit x and y.
    fn touch(&mut self) -> Option<Point> {
        let mut b = [0u8; 6];
        self.i2c
            .write_read(TOUCH, &[0x01], &mut b, I2C_TIMEOUT.ticks())
            .ok()?;
        let coord = |hi: u8, lo: u8| i32::from(hi & 0x0F) << 8 | i32::from(lo);
        (b[1] & 0x0F != 0).then(|| Point::new(coord(b[2], b[3]), coord(b[4], b[5])))
    }

    /// Let `draw` render into the back buffer, then flip.
    pub fn present(&mut self, draw: impl FnOnce(&mut Framebuffer565<'_>)) -> Result<(), EspError> {
        draw(&mut Framebuffer565::new(self.panel.back_buffer(), SCREEN));
        self.panel.present()
    }
}
