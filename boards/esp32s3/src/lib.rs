//! ESP32-S3 backend for game-toolkit-handheld: an ILI9341 320x240 SPI panel and six buttons.
//!
//! Shared by the `maze-embassy` and `maze-blocking` binaries; they differ only in how they
//! pace [`Board::frame`].
//!
//! # Pins
//!
//! Every pin is set in [`Board::new`]; `diagram.json` (Wokwi) wires the same ones.
//!
//! | Signal | GPIO | | Button | GPIO |
//! |--------|------|-|--------|------|
//! | SCK    | 12   | | Up     | 4    |
//! | MOSI   | 11   | | Down   | 5    |
//! | CS     | 10   | | Left   | 6    |
//! | DC     | 9    | | Right  | 7    |
//! | RST    | 8    | | A      | 15   |
//! |        |      | | B      | 16   |
//!
//! Buttons pull up internally and read active low (pressed connects to GND). The panel's
//! backlight (LED) goes to 3V3.
#![no_std]
#![forbid(unsafe_code)]

use embedded_hal_bus::spi::ExclusiveDevice;
use esp_hal::{
    Blocking,
    delay::Delay,
    dma::{DmaDescriptor, DmaRxBuf, DmaTxBuf, aligned::DmaAlignedMut},
    gpio::{Input, InputConfig, Level, Output, OutputConfig, Pull},
    peripherals::{FROM_CPU_INTR0, Peripherals, TIMG0},
    spi::{
        Mode,
        master::{Config, Spi, SpiDma},
    },
    time::{Instant, Rate},
};
use game_toolkit_handheld::{
    Button, Buttons, FrameClock, Framebuffer565, HandheldGame, InputState,
    embedded_graphics::pixelcolor::{Rgb565, raw::RawU16},
    screen,
};
use mipidsi::{
    Builder,
    interface::SpiInterface,
    models::ILI9341Rgb565,
    options::{ColorOrder, Orientation, Rotation},
};
use static_cell::ConstStaticCell;

/// Pixels in one QVGA frame.
const PIXELS: usize = (screen::QVGA.width * screen::QVGA.height) as usize;

/// The frame lives in internal SRAM as a static: 153.6 KB, too big for a task stack.
static FRAME: ConstStaticCell<[u16; PIXELS]> = ConstStaticCell::new([0; PIXELS]);
/// mipidsi packs pixels into this (big-endian, as the panel wants) before each SPI write.
static SPI_CHUNK: ConstStaticCell<[u8; CHUNK]> = ConstStaticCell::new([0; CHUNK]);

/// One DMA descriptor's worth (`esp_hal::dma::CHUNK_SIZE`).
const CHUNK: usize = 4092;

/// DMA wants word-aligned buffers in internal RAM.
#[repr(C, align(4))]
struct Aligned<const N: usize>([u8; N]);

// Built by hand instead of `dma_buffers!`, which expands to `static mut` and `unsafe`.
static DMA_TX: ConstStaticCell<Aligned<CHUNK>> = ConstStaticCell::new(Aligned([0; CHUNK]));
static DMA_RX: ConstStaticCell<Aligned<4>> = ConstStaticCell::new(Aligned([0; 4]));
static DMA_TX_DESC: ConstStaticCell<[DmaDescriptor; 1]> =
    ConstStaticCell::new([DmaDescriptor::EMPTY]);
static DMA_RX_DESC: ConstStaticCell<[DmaDescriptor; 1]> =
    ConstStaticCell::new([DmaDescriptor::EMPTY]);

type Panel = mipidsi::Display<
    SpiInterface<
        'static,
        ExclusiveDevice<SpiDma<'static, Blocking>, Output<'static>, Delay>,
        Output<'static>,
    >,
    ILI9341Rgb565,
    Output<'static>,
>;

/// Peripherals [`Board`] leaves alone, for the runtime (esp-rtos needs both).
pub struct Spare {
    /// Timer group 0.
    pub timg0: TIMG0<'static>,
    /// Software interrupt 0.
    pub int0: FROM_CPU_INTR0<'static>,
}

/// The game's hardware: panel, buttons, framebuffer and frame clock.
pub struct Board {
    panel: Panel,
    frame: &'static mut [u16; PIXELS],
    /// In [`Button`] order: Up, Down, Left, Right, A, B.
    buttons: [(Button, Input<'static>); 6],
    input: game_toolkit_handheld::Input,
    clock: FrameClock,
}

impl Board {
    /// Bring up the panel and buttons; call once, with the peripherals from `esp_hal::init`.
    /// Returns the peripherals a runtime may still need.
    pub fn new(p: Peripherals) -> (Self, Spare) {
        let pull_up = InputConfig::default().with_pull(Pull::Up);
        let buttons = [
            (Button::Up, Input::new(p.GPIO4, pull_up)),
            (Button::Down, Input::new(p.GPIO5, pull_up)),
            (Button::Left, Input::new(p.GPIO6, pull_up)),
            (Button::Right, Input::new(p.GPIO7, pull_up)),
            (Button::A, Input::new(p.GPIO15, pull_up)),
            (Button::B, Input::new(p.GPIO16, pull_up)),
        ];

        let cs = Output::new(p.GPIO10, Level::High, OutputConfig::default());
        let dc = Output::new(p.GPIO9, Level::Low, OutputConfig::default());
        let rst = Output::new(p.GPIO8, Level::High, OutputConfig::default());

        let tx = DmaTxBuf::new(
            DmaAlignedMut::new(DMA_TX_DESC.take().as_mut_slice()).expect("tx desc"),
            DmaAlignedMut::new(DMA_TX.take().0.as_mut_slice()).expect("tx buf"),
        )
        .expect("tx");
        let rx = DmaRxBuf::new(
            DmaAlignedMut::new(DMA_RX_DESC.take().as_mut_slice()).expect("rx desc"),
            DmaAlignedMut::new(DMA_RX.take().0.as_mut_slice()).expect("rx buf"),
        )
        .expect("rx");
        let spi = Spi::new(
            p.SPI2,
            Config::default()
                .with_frequency(Rate::from_mhz(40))
                .with_mode(Mode::_0),
        )
        .expect("SPI config")
        .with_sck(p.GPIO12)
        .with_mosi(p.GPIO11)
        .with_dma(p.DMA_CH0)
        .with_buffers(rx, tx);
        let spi = ExclusiveDevice::new(spi, cs, Delay::new()).expect("SPI CS pin");

        let panel = Builder::new(ILI9341Rgb565, SpiInterface::new(spi, dc, SPI_CHUNK.take()))
            .reset_pin(rst)
            // Landscape, 320x240.
            .orientation(Orientation::new().rotate(Rotation::Deg90).flip_horizontal())
            .color_order(ColorOrder::Bgr)
            .init(&mut Delay::new())
            .expect("ILI9341 init");

        let board = Self {
            panel,
            frame: FRAME.take(),
            buttons,
            input: game_toolkit_handheld::Input::default(),
            clock: FrameClock::new(),
        };
        let spare = Spare {
            timg0: p.TIMG0,
            int0: p.FROM_CPU_INTR0,
        };
        (board, spare)
    }

    /// Run one frame: read buttons, `advance` input, `update`, `render`, push to the panel.
    pub fn frame<G: HandheldGame<Color = Rgb565>>(&mut self, game: &mut G) {
        let mut buttons = Buttons::NONE;
        for (button, pin) in &self.buttons {
            buttons.set(*button, pin.is_low());
        }
        self.input.advance(InputState {
            buttons,
            ..InputState::default()
        });
        let dt = self
            .clock
            .tick(Instant::now().duration_since_epoch().as_micros());
        game.update(&self.input, dt);
        game.render(&mut Framebuffer565::new(self.frame, screen::QVGA));

        let (w, h) = (screen::QVGA.width as u16, screen::QVGA.height as u16);
        let pixels = self.frame.iter().map(|&raw| Rgb565::from(RawU16::new(raw)));
        // SPI errors on this bus are infallible in practice; drop the frame rather than panic.
        let _ = self.panel.set_pixels(0, 0, w - 1, h - 1, pixels);
    }
}
