use core::convert::Infallible;

use embedded_graphics::{
    Pixel,
    draw_target::DrawTarget,
    geometry::{Dimensions, OriginDimensions, Point, Size},
    pixelcolor::{BinaryColor, Rgb565, raw::RawU16},
    prelude::RawData,
    primitives::Rectangle,
};

/// Index of `p` in a row-major `size` buffer, `None` outside it.
fn index(p: Point, size: Size, stride: usize) -> Option<(usize, usize)> {
    let x = u32::try_from(p.x).ok()?;
    let y = u32::try_from(p.y).ok()?;
    (x < size.width && y < size.height).then_some((x as usize, y as usize * stride))
}

/// An RGB565 framebuffer over a borrowed `u16` slice, row-major, one `u16` per pixel in native
/// byte order. Backends that push it over SPI swap bytes for the panel themselves.
#[derive(Debug)]
pub struct Framebuffer565<'a> {
    pixels: &'a mut [u16],
    size: Size,
}

impl<'a> Framebuffer565<'a> {
    /// Wrap `pixels` as a `size` framebuffer.
    ///
    /// # Panics
    /// If `pixels` holds fewer than `size.width * size.height` entries.
    pub fn new(pixels: &'a mut [u16], size: Size) -> Self {
        let len = size.width as usize * size.height as usize;
        assert!(
            pixels.len() >= len,
            "framebuffer of {size:?} needs {len} pixels, got {}",
            pixels.len()
        );
        Self {
            pixels: &mut pixels[..len],
            size,
        }
    }

    /// The pixels, row-major.
    pub fn pixels(&self) -> &[u16] {
        self.pixels
    }
}

impl OriginDimensions for Framebuffer565<'_> {
    fn size(&self) -> Size {
        self.size
    }
}

impl DrawTarget for Framebuffer565<'_> {
    type Color = Rgb565;
    type Error = Infallible;

    fn draw_iter<I: IntoIterator<Item = Pixel<Rgb565>>>(
        &mut self,
        pixels: I,
    ) -> Result<(), Infallible> {
        let width = self.size.width as usize;
        for Pixel(p, color) in pixels {
            if let Some((x, row)) = index(p, self.size, width) {
                self.pixels[row + x] = RawU16::from(color).into_inner();
            }
        }
        Ok(())
    }

    fn fill_solid(&mut self, area: &Rectangle, color: Rgb565) -> Result<(), Infallible> {
        let area = area.intersection(&self.bounding_box());
        let Some(bottom_right) = area.bottom_right() else {
            return Ok(());
        };
        let raw = RawU16::from(color).into_inner();
        let width = self.size.width as usize;
        // Inside the bounding box, so both corners are non-negative.
        let (x0, x1) = (area.top_left.x as usize, bottom_right.x as usize);
        for y in area.top_left.y as usize..=bottom_right.y as usize {
            self.pixels[y * width + x0..=y * width + x1].fill(raw);
        }
        Ok(())
    }

    fn clear(&mut self, color: Rgb565) -> Result<(), Infallible> {
        self.pixels.fill(RawU16::from(color).into_inner());
        Ok(())
    }
}

/// A 1-bit framebuffer over a borrowed byte slice: rows of `stride` bytes, most significant bit
/// leftmost, bit set = [`BinaryColor::On`] = white.
///
/// That is the Playdate's own frame layout (`getFrame()`, 52-byte rows for 400 pixels), so its
/// backend draws straight into the display buffer.
#[derive(Debug)]
pub struct Framebuffer1<'a> {
    bytes: &'a mut [u8],
    size: Size,
    stride: usize,
}

impl<'a> Framebuffer1<'a> {
    /// Wrap `bytes` as a `size` framebuffer with rows of `stride` bytes.
    ///
    /// # Panics
    /// If a row of `stride` bytes can't hold `size.width` bits, or `bytes` holds fewer than
    /// `stride * size.height` bytes.
    pub fn new(bytes: &'a mut [u8], size: Size, stride: usize) -> Self {
        assert!(
            stride * 8 >= size.width as usize,
            "stride {stride} is too short for {} pixels",
            size.width
        );
        let len = stride * size.height as usize;
        assert!(
            bytes.len() >= len,
            "framebuffer of {size:?} needs {len} bytes, got {}",
            bytes.len()
        );
        Self {
            bytes: &mut bytes[..len],
            size,
            stride,
        }
    }

    /// The bytes, row-major, `stride` bytes per row.
    pub fn bytes(&self) -> &[u8] {
        self.bytes
    }

    /// Bytes per row.
    pub fn stride(&self) -> usize {
        self.stride
    }
}

impl OriginDimensions for Framebuffer1<'_> {
    fn size(&self) -> Size {
        self.size
    }
}

impl DrawTarget for Framebuffer1<'_> {
    type Color = BinaryColor;
    type Error = Infallible;

    fn draw_iter<I: IntoIterator<Item = Pixel<BinaryColor>>>(
        &mut self,
        pixels: I,
    ) -> Result<(), Infallible> {
        for Pixel(p, color) in pixels {
            if let Some((x, row)) = index(p, self.size, self.stride) {
                let mask = 0x80 >> (x % 8);
                let byte = &mut self.bytes[row + x / 8];
                if color.is_on() {
                    *byte |= mask;
                } else {
                    *byte &= !mask;
                }
            }
        }
        Ok(())
    }

    fn clear(&mut self, color: BinaryColor) -> Result<(), Infallible> {
        self.bytes.fill(if color.is_on() { 0xFF } else { 0x00 });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use embedded_graphics::{
        Drawable,
        pixelcolor::RgbColor,
        prelude::Primitive,
        primitives::{PrimitiveStyle, Rectangle},
    };

    use super::*;

    #[test]
    fn rgb565_fill_clips_to_the_screen() {
        let mut pixels = [0u16; 4 * 3];
        let mut fb = Framebuffer565::new(&mut pixels, Size::new(4, 3));
        Rectangle::new(Point::new(2, -1), Size::new(10, 3))
            .into_styled(PrimitiveStyle::with_fill(Rgb565::WHITE))
            .draw(&mut fb)
            .unwrap();
        let w = 0xFFFF;
        assert_eq!(fb.pixels(), &[0, 0, w, w, 0, 0, w, w, 0, 0, 0, 0]);
    }

    #[test]
    fn rgb565_ignores_pixels_outside() {
        let mut pixels = [0u16; 4];
        let mut fb = Framebuffer565::new(&mut pixels, Size::new(2, 2));
        let off =
            [(-1, 0), (0, -1), (2, 0), (0, 2)].map(|(x, y)| Pixel(Point::new(x, y), Rgb565::RED));
        fb.draw_iter(off).unwrap();
        assert_eq!(fb.pixels(), &[0; 4]);
    }

    #[test]
    fn binary_is_msb_first_with_stride_padding() {
        // 10 pixels wide, 2-byte rows plus 1 padding byte, like the Playdate's 52-byte rows.
        let mut bytes = [0u8; 3 * 2];
        let mut fb = Framebuffer1::new(&mut bytes, Size::new(10, 2), 3);
        let on = [(0, 0), (9, 0), (1, 1)].map(|(x, y)| Pixel(Point::new(x, y), BinaryColor::On));
        fb.draw_iter(on).unwrap();
        fb.draw_iter([Pixel(Point::new(10, 0), BinaryColor::On)])
            .unwrap(); // past width
        assert_eq!(fb.bytes(), &[0x80, 0x40, 0, 0x40, 0, 0]);
        fb.draw_iter([Pixel(Point::new(0, 0), BinaryColor::Off)])
            .unwrap();
        assert_eq!(fb.bytes()[0], 0);
    }

    #[test]
    #[should_panic(expected = "too short")]
    fn binary_rejects_short_stride() {
        let mut bytes = [0u8; 8];
        Framebuffer1::new(&mut bytes, Size::new(9, 2), 1);
    }
}
