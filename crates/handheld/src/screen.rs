//! Screen sizes of the supported devices, for [`HandheldGame::init`](crate::HandheldGame::init)
//! and the simulator.

use embedded_graphics::geometry::Size;

/// Kode Dot: 2.13" AMOLED, ESP32-P4. Portrait: the panel scans 410 wide by 502 tall and has no
/// hardware rotation, so landscape would cost a software rotate every frame. The corners are
/// rounded (radius 114 px), so keep important pixels away from them.
pub const KODE_DOT: Size = Size::new(410, 502);
/// 320x240, the common ST7789 / ILI9341 SPI panel on ESP32-S3 boards.
pub const QVGA: Size = Size::new(320, 240);
/// Playdate: 1-bit Sharp memory LCD.
pub const PLAYDATE: Size = Size::new(400, 240);
