//! Buttons and touch, shared by the real board and Wokwi (which runs the TCA6408A and CST820
//! as custom chips on the same pins): D-pad and BOT on the expander, TOP on GPIO35, touch on
//! the CST820, both chips on I2C0 (SDA GPIO4, SCL GPIO5).

use cst820::Cst820;
use embedded_hal::i2c::{ErrorType, I2c, Operation};
use esp_idf_svc::{
    hal::{
        delay::TickType,
        gpio::{self, Gpio4, Gpio5, Gpio35, PinDriver, Pull},
        i2c::{I2C0, I2cConfig, I2cDriver, I2cError},
        units::FromValueType,
    },
    sys::EspError,
};
use game_toolkit_handheld::{Button, Buttons, InputState, embedded_graphics::prelude::Point};
use tca6408a::Tca6408a;

/// Expander P7 drives VCI_EN, the panel supply; P0..P6 are inputs.
const VCI_EN: u8 = 1 << 7;
/// Expander inputs, active low. Holding BOT is kodeOS's home gesture, so B never needs a hold.
const PAD: [(u8, Button); 5] = [
    (0, Button::B),
    (1, Button::Up),
    (2, Button::Left),
    (3, Button::Down),
    (4, Button::Right),
];
/// A stuck bus costs a frame, not the game.
const I2C_TIMEOUT: TickType = TickType::new_millis(10);

/// I2C0 with [`I2C_TIMEOUT`]: esp-idf-hal's own embedded-hal impl waits forever.
struct Bus(I2cDriver<'static>);

impl ErrorType for Bus {
    type Error = I2cError;
}

impl I2c for Bus {
    fn transaction(&mut self, address: u8, ops: &mut [Operation<'_>]) -> Result<(), I2cError> {
        self.0
            .transaction(address, ops, I2C_TIMEOUT.ticks())
            .map_err(I2cError::other)
    }
}

/// The expander, the touch controller and the TOP button.
pub struct Controls {
    bus: Bus,
    top: PinDriver<'static, gpio::Input>,
}

impl Controls {
    /// I2C0 at 400 kHz and TOP with a pull-up. Performs no bus access.
    pub fn new(
        i2c: I2C0<'static>,
        sda: Gpio4<'static>,
        scl: Gpio5<'static>,
        top: Gpio35<'static>,
    ) -> Result<Self, EspError> {
        let config = I2cConfig::new().baudrate(400.kHz().into());
        Ok(Self {
            bus: Bus(I2cDriver::new(i2c, sda, scl, &config)?),
            top: PinDriver::input(top, Pull::Up)?,
        })
    }

    /// Switch the panel supply on via expander P7, without a glitch low.
    pub fn power_panel(&mut self) -> Result<(), EspError> {
        Tca6408a::new(&mut self.bus)
            .make_outputs(VCI_EN, VCI_EN)
            .map_err(|e| e.cause())
    }

    /// Buttons and touch, read now.
    pub fn read(&mut self) -> InputState {
        let mut buttons = Buttons::NONE;
        if let Ok(levels) = Tca6408a::new(&mut self.bus).read_inputs() {
            for (bit, button) in PAD {
                buttons.set(button, levels & (1 << bit) == 0);
            }
        }
        buttons.set(Button::A, self.top.is_low());
        // The CST820 NAKs while it dozes, which reads as no touch.
        let touch = Cst820::new(&mut self.bus)
            .read_touch()
            .ok()
            .filter(cst820::Touch::pressed)
            .map(|t| Point::new(t.x.into(), t.y.into()));
        InputState {
            buttons,
            touch,
            ..InputState::default()
        }
    }
}
