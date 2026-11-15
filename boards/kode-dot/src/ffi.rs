//! The crate's only `unsafe`: esp_lcd calls that bring up the CO5300 over MIPI-DSI and hand out
//! its two PSRAM framebuffers. esp-idf-hal's `LcdDriver` can't be used: its DPI callback lives in
//! flash, which `CONFIG_LCD_DSI_ISR_CACHE_SAFE` rejects.
#![allow(unsafe_code)]

use core::{
    ffi::c_void,
    ptr,
    sync::atomic::{AtomicU32, Ordering},
};
use std::time::Duration;

use esp_idf_svc::sys::{self, EspError, esp};

/// Visible pixels.
pub const WIDTH: u16 = 410;
/// Visible lines.
pub const HEIGHT: u16 = 502;
/// The glass sits centred in the panel's 416-column memory, starting at this column.
const COL_OFFSET: u16 = 22;

/// DPI scan-outs finished, counted in the ISR: the DMA restarts on the current front buffer
/// right then, so the previous one is free from the next count on.
static SCANOUTS: AtomicU32 = AtomicU32::new(0);

/// `on_frame_buf_complete`, called from the DSI ISR.
// SAFETY: `link_section` puts it in IRAM, which `CONFIG_LCD_DSI_ISR_CACHE_SAFE` requires (the ISR
// may run with the flash cache off). The body is one atomic add (`amoadd.w`), so it calls
// nothing that lives in flash.
#[unsafe(link_section = ".iram1.kode_dot_scanout")]
unsafe extern "C" fn on_scanout(
    _panel: sys::esp_lcd_panel_handle_t,
    _edata: *mut sys::esp_lcd_dpi_panel_event_data_t,
    _ctx: *mut c_void,
) -> bool {
    SCANOUTS.fetch_add(1, Ordering::Release);
    false
}

/// The CO5300 panel in DPI video mode with two framebuffers. Never deleted: it lives as long
/// as the app.
pub struct Panel {
    panel: sys::esp_lcd_panel_handle_t,
    fbs: [*mut u16; 2],
    back: usize,
}

impl Panel {
    /// DSI bus, DBI command IO, then the CO5300 (which creates the DPI panel). Expects the
    /// panel supply on, the reset pulse done and the DSI PHY LDO up.
    pub fn new() -> Result<Self, EspError> {
        let bus_config = sys::esp_lcd_dsi_bus_config_t {
            bus_id: 0,
            num_data_lanes: 1,
            phy_clk_src: 0, // driver default
            lane_bit_rate_mbps: 360.0,
            ..Default::default()
        };
        let mut bus = ptr::null_mut();
        // SAFETY: both pointers are valid for the call; the driver copies the config.
        esp!(unsafe { sys::esp_lcd_new_dsi_bus(&bus_config, &mut bus) })?;

        let dbi_config = sys::esp_lcd_dbi_io_config_t {
            virtual_channel: 0,
            lcd_cmd_bits: 8,
            lcd_param_bits: 8,
        };
        let mut io = ptr::null_mut();
        // SAFETY: `bus` is the live bus from above; the pointers are valid for the call.
        esp!(unsafe { sys::esp_lcd_new_panel_io_dbi(bus, &dbi_config, &mut io) })?;

        let dpi_config = sys::esp_lcd_dpi_panel_config_t {
            virtual_channel: 0,
            dpi_clk_src: sys::soc_periph_mipi_dsi_dpi_clk_src_t_MIPI_DSI_DPI_CLK_SRC_DEFAULT,
            dpi_clock_freq_mhz: 13.0,
            in_color_format: sys::lcd_color_format_t_LCD_COLOR_FMT_RGB565,
            num_fbs: 2,
            video_timing: sys::esp_lcd_video_timing_t {
                h_size: WIDTH.into(),
                v_size: HEIGHT.into(),
                hsync_pulse_width: 2,
                hsync_back_porch: 5,
                hsync_front_porch: 7,
                vsync_pulse_width: 2,
                vsync_back_porch: 5,
                vsync_front_porch: 7,
            },
            ..Default::default()
        };
        let mut vendor = sys::co5300_vendor_config_t::default();
        vendor.__bindgen_anon_1.mipi_config.dsi_bus = bus;
        vendor.__bindgen_anon_1.mipi_config.dpi_config = &dpi_config;
        vendor.flags.set_use_mipi_interface(1);
        let dev_config = sys::esp_lcd_panel_dev_config_t {
            reset_gpio_num: -1, // GPIO7 is shared with touch; the board pulses it before this
            bits_per_pixel: 16,
            vendor_config: (&raw mut vendor).cast(),
            ..Default::default()
        };
        let mut panel = ptr::null_mut();
        // SAFETY: `io` and `bus` are live; `vendor` and `dpi_config` outlive the call, which
        // copies what it keeps.
        esp!(unsafe { sys::esp_lcd_new_panel_co5300(io, &dev_config, &mut panel) })?;
        // SAFETY: `panel` is the live handle from above (reset is a SWRESET without a pin).
        esp!(unsafe { sys::esp_lcd_panel_reset(panel) })?;
        // SAFETY: as above; init sends the vendor sequence and starts DPI video.
        esp!(unsafe { sys::esp_lcd_panel_init(panel) })?;

        let [c0, c1] = COL_OFFSET.to_be_bytes();
        let [c2, c3] = (COL_OFFSET + WIDTH - 1).to_be_bytes();
        let [r2, r3] = (HEIGHT - 1).to_be_bytes();
        let commands: [(u32, &[u8]); 3] = [
            (sys::LCD_CMD_CASET, &[c0, c1, c2, c3]),
            (sys::LCD_CMD_RASET, &[0, 0, r2, r3]),
            (0x51, &[0xFF]), // brightness, 0..=255
        ];
        for (cmd, params) in commands {
            // SAFETY: `io` is live; `params` is valid for `params.len()` bytes during the call.
            esp!(unsafe {
                sys::esp_lcd_panel_io_tx_param(io, cmd as i32, params.as_ptr().cast(), params.len())
            })?;
        }

        let mut callbacks = sys::esp_lcd_dpi_panel_event_callbacks_t::default();
        callbacks.__bindgen_anon_1.on_frame_buf_complete = Some(on_scanout);
        // SAFETY: `panel` is the DPI panel the CO5300 driver created; the callback is in IRAM
        // and ignores its context pointer.
        esp!(unsafe {
            sys::esp_lcd_dpi_panel_register_event_callbacks(panel, &callbacks, ptr::null_mut())
        })?;

        let (mut fb0, mut fb1) = (ptr::null_mut::<c_void>(), ptr::null_mut::<c_void>());
        // SAFETY: the panel was created with `num_fbs: 2`, so the driver writes two pointers.
        esp!(unsafe { sys::esp_lcd_dpi_panel_get_frame_buffer(panel, 2, &mut fb0, &mut fb1) })?;

        Ok(Self {
            panel,
            fbs: [fb0.cast(), fb1.cast()],
            back: 1, // the driver scans out fb0 first
        })
    }

    /// The framebuffer that is not on screen: `WIDTH * HEIGHT` RGB565 pixels, row-major.
    pub fn back_buffer(&mut self) -> &mut [u16] {
        // SAFETY: the driver allocated each buffer as WIDTH * HEIGHT * 2 bytes, aligned for DMA,
        // and never frees it (the panel is never deleted). The DMA reads only the front buffer
        // (`present` waits for it to switch), and `&mut self` makes this the only reference.
        unsafe {
            std::slice::from_raw_parts_mut(
                self.fbs[self.back],
                usize::from(WIDTH) * usize::from(HEIGHT),
            )
        }
    }

    /// Show the back buffer from the next scan-out on, then wait until the old front buffer is
    /// no longer read, so it can be drawn into.
    pub fn present(&mut self) -> Result<(), EspError> {
        let fb = self.fbs[self.back];
        // SAFETY: `fb` is one of the panel's own framebuffers, so this writes back the cache
        // and switches buffers instead of copying.
        esp!(unsafe {
            sys::esp_lcd_panel_draw_bitmap(self.panel, 0, 0, WIDTH.into(), HEIGHT.into(), fb.cast())
        })?;
        self.back ^= 1;
        // Read after the switch: the next scan-out end restarts the DMA on the new buffer.
        let switched = SCANOUTS.load(Ordering::Acquire);
        while SCANOUTS.load(Ordering::Acquire) == switched {
            std::thread::sleep(Duration::from_millis(1));
        }
        Ok(())
    }
}
