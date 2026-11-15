//! Wokwi stand-in: Wokwi has no CO5300, so an ILI9341 on SPI2 shows a 320x240 screen. Buttons
//! and touch run on the TCA6408A and CST820 custom chips, wired like the real board
//! (`diagram.json`).

use std::{thread::sleep, time::Duration};

use esp_idf_svc::{
    hal::{
        gpio::{self, AnyIOPin, PinDriver},
        peripherals::Peripherals,
        spi::{SpiDeviceDriver, SpiDriver, SpiDriverConfig, config::Config},
        units::FromValueType,
    },
    sys::EspError,
};
use game_toolkit_handheld::{
    Framebuffer565, InputState, embedded_graphics::geometry::Size, screen,
};

use crate::controls::Controls;

/// Landscape QVGA, the ILI9341 rotated.
pub const SCREEN: Size = screen::QVGA;
/// Rows per SPI write.
const BAND: usize = 24;

/// Everything the game loop needs from the simulated board.
pub struct Board {
    spi: SpiDeviceDriver<'static, SpiDriver<'static>>,
    dc: PinDriver<'static, gpio::Output>,
    _reset: PinDriver<'static, gpio::Output>,
    /// Held high, like the real board's shared display + touch reset; here it only reaches touch.
    _touch_reset: PinDriver<'static, gpio::Output>,
    controls: Controls,
    pixels: Vec<u16>,
    band: Vec<u8>,
}

impl Board {
    /// SPI2 on GPIO8 (SCK), GPIO9 (MOSI), GPIO10 (CS); DC on GPIO11, reset on GPIO12. Touch
    /// reset on GPIO7, as on the real board.
    pub fn new(p: Peripherals) -> Result<Self, EspError> {
        let mut controls = Controls::new(p.i2c0, p.pins.gpio4, p.pins.gpio5, p.pins.gpio35)?;
        controls.power_panel()?; // only lights the VCI_EN LED here
        let mut touch_reset = PinDriver::output(p.pins.gpio7)?;
        touch_reset.set_low()?;
        sleep(Duration::from_millis(10));
        touch_reset.set_high()?;

        let spi = SpiDeviceDriver::new_single(
            p.spi2,
            p.pins.gpio8,
            p.pins.gpio9,
            None::<AnyIOPin<'static>>,
            Some(p.pins.gpio10),
            &SpiDriverConfig::new(),
            &Config::new().baudrate(40.MHz().into()),
        )?;
        let mut reset = PinDriver::output(p.pins.gpio12)?;
        reset.set_low()?;
        sleep(Duration::from_millis(10));
        reset.set_high()?;
        sleep(Duration::from_millis(120));

        let mut board = Self {
            spi,
            dc: PinDriver::output(p.pins.gpio11)?,
            _reset: reset,
            _touch_reset: touch_reset,
            controls,
            pixels: vec![0; (SCREEN.width * SCREEN.height) as usize],
            band: Vec::with_capacity(SCREEN.width as usize * BAND * 2),
        };
        board.command(0x11, &[])?; // sleep out
        sleep(Duration::from_millis(120));
        board.command(0x3A, &[0x55])?; // 16 bits per pixel
        board.command(0x36, &[0x28])?; // row/column exchange (landscape), BGR
        board.command(0x29, &[])?; // display on
        Ok(board)
    }

    fn command(&mut self, cmd: u8, params: &[u8]) -> Result<(), EspError> {
        self.dc.set_low()?;
        self.spi.write(&[cmd])?;
        self.dc.set_high()?;
        if !params.is_empty() {
            self.spi.write(params)?;
        }
        Ok(())
    }

    /// Buttons and touch, read now.
    pub fn input(&mut self) -> InputState {
        self.controls.read()
    }

    /// Let `draw` render into the framebuffer, then send it all (big-endian on the wire).
    pub fn present(&mut self, draw: impl FnOnce(&mut Framebuffer565<'_>)) -> Result<(), EspError> {
        draw(&mut Framebuffer565::new(&mut self.pixels, SCREEN));
        let [x1, x2] = (SCREEN.width as u16 - 1).to_be_bytes();
        let [y1, y2] = (SCREEN.height as u16 - 1).to_be_bytes();
        self.command(0x2A, &[0, 0, x1, x2])?;
        self.command(0x2B, &[0, 0, y1, y2])?;
        self.command(0x2C, &[])?;
        for rows in self.pixels.chunks(SCREEN.width as usize * BAND) {
            self.band.clear();
            self.band.extend(rows.iter().flat_map(|p| p.to_be_bytes()));
            self.spi.write(&self.band)?;
        }
        Ok(())
    }
}
