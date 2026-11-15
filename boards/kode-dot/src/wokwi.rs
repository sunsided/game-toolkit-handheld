//! Wokwi stand-in: Wokwi has no MIPI-DSI, so the CO5300 custom chip sits on SPI2 instead and
//! gets the same 410x502 screen and the same commands through the `co5300` crate. Buttons and
//! touch run on the TCA6408A and CST820 custom chips. Everything is wired like the real board
//! where it can be (`diagram.json`).

use co5300::{Co5300, Config, SpiInterface};
use esp_idf_svc::{
    hal::{
        delay::Delay,
        gpio::{self, AnyIOPin, PinDriver},
        peripherals::Peripherals,
        spi::{SpiDeviceDriver, SpiDriver, SpiDriverConfig, config::Config as SpiConfig},
        units::FromValueType,
    },
    sys::{self, EspError},
};
use game_toolkit_handheld::{Framebuffer565, InputState};

use crate::{SCREEN, controls::Controls};

/// Rows per SPI write.
const BAND: usize = 24;

type Panel = Co5300<
    SpiInterface<SpiDeviceDriver<'static, SpiDriver<'static>>, PinDriver<'static, gpio::Output>>,
>;

/// Everything the game loop needs from the simulated board.
pub struct Board {
    panel: Panel,
    /// Held high: dropping it would float the display + touch reset line.
    _reset: PinDriver<'static, gpio::Output>,
    controls: Controls,
    pixels: Vec<u16>,
    band: Vec<u8>,
}

impl Board {
    /// SPI2 on GPIO8 (SCL), GPIO9 (SDI), GPIO10 (CSX); DCX on GPIO11. Reset on GPIO7, shared
    /// with touch as on the real board.
    pub fn new(p: Peripherals) -> Result<Self, EspError> {
        let mut controls = Controls::new(p.i2c0, p.pins.gpio4, p.pins.gpio5, p.pins.gpio35)?;
        controls.power_panel()?; // only lights the VCI_EN LED here

        let spi = SpiDeviceDriver::new_single(
            p.spi2,
            p.pins.gpio8,
            p.pins.gpio9,
            None::<AnyIOPin<'static>>,
            Some(p.pins.gpio10),
            &SpiDriverConfig::new(),
            &SpiConfig::new().baudrate(40.MHz().into()),
        )?;
        let dc = PinDriver::output(p.pins.gpio11)?;
        let mut reset = PinDriver::output(p.pins.gpio7)?;
        let mut delay = Delay::new_default();
        co5300::reset(&mut reset, &mut delay).map_err(panel_error)?;

        let config = Config::new(SCREEN.width as u16, SCREEN.height as u16);
        let mut panel = Co5300::new(SpiInterface::new(spi, dc), config);
        panel.init(&mut delay, &[]).map_err(panel_error)?;
        Ok(Self {
            panel,
            _reset: reset,
            controls,
            pixels: vec![0; (SCREEN.width * SCREEN.height) as usize],
            band: Vec::with_capacity(SCREEN.width as usize * BAND * 2),
        })
    }

    /// Buttons and touch, read now.
    pub fn input(&mut self) -> InputState {
        self.controls.read()
    }

    /// Let `draw` render into the framebuffer, then send it all (big-endian on the wire).
    pub fn present(&mut self, draw: impl FnOnce(&mut Framebuffer565<'_>)) -> Result<(), EspError> {
        draw(&mut Framebuffer565::new(&mut self.pixels, SCREEN));
        let (w, h) = (SCREEN.width as u16, SCREEN.height as u16);
        self.panel.set_window(0, 0, w, h).map_err(panel_error)?;
        for (i, rows) in self.pixels.chunks(SCREEN.width as usize * BAND).enumerate() {
            self.band.clear();
            self.band.extend(rows.iter().flat_map(|p| p.to_be_bytes()));
            let sent = if i == 0 {
                self.panel.write_pixels(&self.band)
            } else {
                self.panel.write_pixels_continue(&self.band)
            };
            sent.map_err(panel_error)?;
        }
        Ok(())
    }
}

/// The driver's error carries the SPI or pin error; log it, the game loop only takes an
/// `EspError`.
fn panel_error(e: impl core::fmt::Debug) -> EspError {
    log::error!("CO5300: {e:?}");
    EspError::from_infallible::<{ sys::ESP_FAIL }>()
}
