//! Run a [`HandheldGame`] in a game-toolkit window, natively or in the browser.
//!
//! Keys: arrows or WASD for the D-pad, X / Space for A, Z for B, Enter for Menu, Q / E or the
//! mouse wheel for the crank, left mouse button for touch, Esc to quit.
//!
//! Natively, `HANDHELD_SHOT=out.png` saves a screenshot of the third frame and quits.

use std::{convert::Infallible, marker::PhantomData, sync::Mutex};

use embedded_graphics::{
    Pixel,
    draw_target::DrawTarget,
    geometry::{OriginDimensions, Point, Size},
    pixelcolor::{BinaryColor, Rgb565, RgbColor},
};
use game_toolkit::prelude::{
    AppConfig, Context, Frame, Game, Key, MouseButton, Result, StreamHandle, TextureId, anyhow,
};

use crate::{AUDIO_RATE, AudioSource, Button, Buttons, Crank, HandheldGame, Input, InputState};

/// A pixel color the simulator can show.
pub trait SimColor: embedded_graphics::pixelcolor::PixelColor {
    /// The color as RGBA8.
    fn rgba(self) -> [u8; 4];
}

impl SimColor for Rgb565 {
    fn rgba(self) -> [u8; 4] {
        // Widen 5/6-bit channels by repeating their top bits, so full intensity stays 255.
        let (r, g, b) = (self.r(), self.g(), self.b());
        [r << 3 | r >> 2, g << 2 | g >> 4, b << 3 | b >> 2, 255]
    }
}

impl SimColor for BinaryColor {
    fn rgba(self) -> [u8; 4] {
        // The Playdate's grey-white and near-black, not pure white on black.
        match self {
            BinaryColor::On => [0xB1, 0xAF, 0xA8, 255],
            BinaryColor::Off => [0x32, 0x2F, 0x28, 255],
        }
    }
}

/// Simulator settings.
#[derive(Clone, Debug)]
pub struct SimConfig {
    /// Window title.
    pub title: String,
    /// Device screen in pixels, e.g. [`crate::screen::PLAYDATE`].
    pub screen: Size,
    /// Window pixels per device pixel.
    pub scale: u32,
}

impl SimConfig {
    /// `screen` at scale 2.
    pub fn new(title: impl Into<String>, screen: Size) -> Self {
        Self {
            title: title.into(),
            screen,
            scale: 2,
        }
    }
}

/// Handed from [`run`] to [`Runner::init`], which game-toolkit calls without arguments.
static PENDING: Mutex<Option<SimConfig>> = Mutex::new(None);

/// Open a window and run `G` in it until Esc or the window closes.
pub fn run<G>(config: SimConfig) -> Result<()>
where
    G: HandheldGame,
    G::Color: SimColor,
{
    #[cfg(not(target_arch = "wasm32"))]
    eprintln!(
        "handheld simulator keys:\n  \
         D-pad   arrows / WASD\n  \
         A       X / Space\n  \
         B       Z\n  \
         Menu    Enter\n  \
         Crank   mouse wheel / Q E\n  \
         Touch   left mouse button\n  \
         Quit    Esc"
    );
    let app = AppConfig {
        title: config.title.clone(),
        width: config.screen.width * config.scale,
        height: config.screen.height * config.scale,
        ..Default::default()
    };
    *PENDING
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(config);
    game_toolkit::prelude::run::<Runner<G>>(app)
}

struct Runner<G> {
    game: G,
    input: Input,
    config: SimConfig,
    texture: TextureId,
    rgba: Vec<u8>,
    crank_angle: f32,
    /// `HANDHELD_SHOT` path and frames rendered so far.
    shot: Option<(String, u32)>,
    /// Keeps the game's sound playing.
    _audio: Option<StreamHandle>,
}

impl<G> Game for Runner<G>
where
    G: HandheldGame,
    G::Color: SimColor,
{
    fn init(ctx: &mut Context) -> Result<Self> {
        let config = PENDING
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
            .ok_or_else(|| anyhow::anyhow!("start the simulator with sim::run"))?;
        let mut game = G::init(config.screen);
        let audio = game.audio().and_then(|source| play(ctx, source));
        let rgba = vec![0; (config.screen.width * config.screen.height * 4) as usize];
        let texture = ctx.gfx.create_texture_rgba(
            config.screen.width,
            config.screen.height,
            &rgba,
            Some("handheld screen"),
        );
        Ok(Self {
            game,
            input: Input::default(),
            config,
            texture,
            rgba,
            crank_angle: 0.0,
            shot: std::env::var("HANDHELD_SHOT").ok().map(|path| (path, 0)),
            _audio: audio,
        })
    }

    fn update(&mut self, ctx: &mut Context, dt: f32) {
        if ctx.input.key_pressed(Key::Escape) {
            ctx.quit();
        }
        let keys = &ctx.input;
        let any = |k: &[Key]| k.iter().any(|&k| keys.key_held(k));
        let mut buttons = Buttons::NONE;
        for (button, k) in [
            (Button::Up, &[Key::ArrowUp, Key::KeyW][..]),
            (Button::Down, &[Key::ArrowDown, Key::KeyS]),
            (Button::Left, &[Key::ArrowLeft, Key::KeyA]),
            (Button::Right, &[Key::ArrowRight, Key::KeyD]),
            (Button::A, &[Key::KeyX, Key::Space]),
            (Button::B, &[Key::KeyZ]),
            (Button::Menu, &[Key::Enter]),
        ] {
            buttons.set(button, any(k));
        }

        // shortcut: 15 degrees per wheel line; trackpads report pixels and turn it much faster.
        let mut delta = keys.scroll().1 * 15.0;
        if any(&[Key::KeyE]) {
            delta += 360.0 * dt;
        }
        if any(&[Key::KeyQ]) {
            delta -= 360.0 * dt;
        }
        self.crank_angle = (self.crank_angle + delta).rem_euclid(360.0);

        let scale = self.config.scale as f32;
        let (mx, my) = keys.mouse_pos();
        let touch = Point::new((mx / scale) as i32, (my / scale) as i32);
        let inside = touch.x >= 0
            && touch.y >= 0
            && (touch.x as u32) < self.config.screen.width
            && (touch.y as u32) < self.config.screen.height;

        self.input.advance(InputState {
            buttons,
            touch: (inside && keys.mouse_held(MouseButton::Left)).then_some(touch),
            motion: None,
            crank: Some(Crank {
                angle: self.crank_angle,
                delta,
            }),
        });
        self.game.update(&self.input, dt);
    }

    fn render(&mut self, ctx: &mut Context, frame: &mut Frame) {
        self.game.render(&mut RgbaTarget {
            rgba: &mut self.rgba,
            size: self.config.screen,
            color: PhantomData,
        });
        ctx.gfx.update_texture_rgba(self.texture, &self.rgba);
        let scale = self.config.scale as f32;
        let mut p = frame.painter(&mut ctx.gfx);
        p.clear([0.0, 0.0, 0.0, 1.0]);
        p.sprite(
            self.texture,
            [0.0, 0.0],
            [
                self.config.screen.width as f32 * scale,
                self.config.screen.height as f32 * scale,
            ],
        );
        drop(p);
        if let Some((path, frames)) = &mut self.shot {
            *frames += 1;
            match *frames {
                3 => ctx.gfx.request_screenshot(path.as_str()),
                5 => ctx.quit(),
                _ => {}
            }
        }
    }
}

/// Start `source` on the toolkit's audio, converting to `f32`. `None` without an audio device.
fn play(ctx: &mut Context, mut source: impl AudioSource) -> Option<StreamHandle> {
    let mut samples = Vec::new();
    let stream = move |out: &mut [f32]| {
        samples.resize(out.len(), 0);
        source.fill(&mut samples);
        for (o, &s) in out.iter_mut().zip(&samples) {
            *o = f32::from(s) / 32768.0;
        }
    };
    match ctx.audio.as_mut()?.play_stream(stream, AUDIO_RATE) {
        Ok(handle) => Some(handle),
        Err(err) => {
            log::warn!("handheld sim: no sound: {err}");
            None
        }
    }
}

/// The simulator's screen: an RGBA8 buffer the game draws into in its own color type.
struct RgbaTarget<'a, C> {
    rgba: &'a mut [u8],
    size: Size,
    color: PhantomData<C>,
}

impl<C> OriginDimensions for RgbaTarget<'_, C> {
    fn size(&self) -> Size {
        self.size
    }
}

impl<C: SimColor> DrawTarget for RgbaTarget<'_, C> {
    type Color = C;
    type Error = Infallible;

    fn draw_iter<I: IntoIterator<Item = Pixel<C>>>(&mut self, pixels: I) -> Result<(), Infallible> {
        for Pixel(p, color) in pixels {
            if let (Ok(x), Ok(y)) = (u32::try_from(p.x), u32::try_from(p.y))
                && x < self.size.width
                && y < self.size.height
            {
                let i = (y * self.size.width + x) as usize * 4;
                self.rgba[i..i + 4].copy_from_slice(&color.rgba());
            }
        }
        Ok(())
    }
}
