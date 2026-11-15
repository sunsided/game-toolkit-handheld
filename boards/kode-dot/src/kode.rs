//! The real Kode Dot: power-up, CO5300 panel, expander buttons, TOP button and CST820 touch.

use std::{thread::sleep, time::Duration};

use esp_idf_svc::{
    hal::{
        gpio::{self, PinDriver},
        ldo::{Adjustable, LdoChannel, config::LdoChannelConfig},
        peripherals::Peripherals,
    },
    sys::EspError,
};
use game_toolkit_handheld::{Framebuffer565, InputState};

use crate::{SCREEN, controls::Controls, ffi::Panel};

/// Everything the game loop needs from the hardware.
pub struct Board {
    controls: Controls,
    /// Held high: dropping it would float the display + touch reset line.
    _reset: PinDriver<'static, gpio::Output>,
    /// Keeps the DSI PHY powered.
    _phy_ldo: LdoChannel<'static, Adjustable>,
    panel: Panel,
}

impl Board {
    /// Power up in the order the board needs: panel supply, reset, PHY LDO, then DSI.
    pub fn new(p: Peripherals) -> Result<Self, EspError> {
        let mut controls = Controls::new(p.i2c0, p.pins.gpio4, p.pins.gpio5, p.pins.gpio35)?;
        controls.power_panel()?;
        sleep(Duration::from_millis(20));

        let mut reset = PinDriver::output(p.pins.gpio7)?;
        reset.set_low()?;
        sleep(Duration::from_millis(10));
        reset.set_high()?;
        sleep(Duration::from_millis(120));

        let phy_ldo = LdoChannel::new(p.ldo3, &LdoChannelConfig::new(2500))?;
        let panel = Panel::new()?;

        Ok(Self {
            controls,
            _reset: reset,
            _phy_ldo: phy_ldo,
            panel,
        })
    }

    /// Buttons and touch, read now.
    pub fn input(&mut self) -> InputState {
        self.controls.read()
    }

    /// Let `draw` render into the back buffer, then flip.
    pub fn present(&mut self, draw: impl FnOnce(&mut Framebuffer565<'_>)) -> Result<(), EspError> {
        draw(&mut Framebuffer565::new(self.panel.back_buffer(), SCREEN));
        self.panel.present()
    }
}
