//! Wokwi stand-in: Wokwi has no CO5300 or TCA6408A, so an ILI9341 on SPI2 shows a 320x240
//! screen and six push buttons sit on plain GPIOs (wiring in `diagram.json`).

use std::{thread::sleep, time::Duration};

use esp_idf_svc::{
    hal::{
        gpio::{self, AnyIOPin, PinDriver, Pull},
        peripherals::Peripherals,
        spi::{SpiDeviceDriver, SpiDriver, SpiDriverConfig, config::Config},
        units::FromValueType,
    },
    sys::EspError,
};
use game_toolkit_handheld::{
    Button, Buttons, Framebuffer565, InputState, embedded_graphics::geometry::Size, screen,
};

/// Landscape QVGA, the ILI9341 rotated.
pub const SCREEN: Size = screen::QVGA;
/// Rows per SPI write.
const BAND: usize = 24;

/// Everything the game loop needs from the simulated board.
pub struct Board {
    spi: SpiDeviceDriver<'static, SpiDriver<'static>>,
    dc: PinDriver<'static, gpio::Output>,
    _reset: PinDriver<'static, gpio::Output>,
    buttons: [(PinDriver<'static, gpio::Input>, Button); 6],
    pixels: Vec<u16>,
    band: Vec<u8>,
}

impl Board {
    /// SPI2 on GPIO8 (SCK), GPIO9 (MOSI), GPIO10 (CS); DC on GPIO11, reset on GPIO12.
    pub fn new(p: Peripherals) -> Result<Self, EspError> {
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

        let button = |pin, b| Ok::<_, EspError>((PinDriver::input(pin, Pull::Up)?, b));
        let mut board = Self {
            spi,
            dc: PinDriver::output(p.pins.gpio11)?,
            _reset: reset,
            buttons: [
                button(AnyIOPin::from(p.pins.gpio20), Button::Up)?,
                button(p.pins.gpio21.into(), Button::Down)?,
                button(p.pins.gpio22.into(), Button::Left)?,
                button(p.pins.gpio23.into(), Button::Right)?,
                button(p.pins.gpio35.into(), Button::A)?,
                button(p.pins.gpio33.into(), Button::B)?,
            ],
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

    /// Push buttons to ground, read now.
    pub fn input(&mut self) -> InputState {
        let mut buttons = Buttons::NONE;
        for (pin, button) in &self.buttons {
            buttons.set(*button, pin.is_low());
        }
        InputState {
            buttons,
            ..InputState::default()
        }
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
