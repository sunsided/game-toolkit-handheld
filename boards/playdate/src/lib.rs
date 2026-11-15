//! Playdate backend for game-toolkit-handheld: runs any 1-bit [`HandheldGame`] from the
//! Playdate's update callback, in the SDK simulator and on the device.
//!
//! ```rust,ignore
//! board_playdate::entry!(game_toolkit_handheld_demos::Dial);
//! ```
#![no_std]

extern crate alloc;

use alloc::boxed::Box;
use core::ffi::{c_int, c_void};

use game_toolkit_handheld::{
    AudioSource, Button, Buttons, Crank, FrameClock, Framebuffer1, HandheldGame, Input, InputState,
    Motion, embedded_graphics::pixelcolor::BinaryColor, screen,
};
use playdate::{
    controls::{
        buttons::PDButtonsExt,
        peripherals::{Accelerometer, Buttons as PdButtons, Crank as PdCrank},
    },
    graphics::Graphics,
    sound::Sound,
    system::{
        System,
        update::{Update, UpdateCtrl},
    },
};

#[doc(hidden)]
pub use playdate::sys::{EventLoopCtrl, ffi::PDSystemEvent, ffi::PlaydateAPI, ll_symbols};

/// Bytes per framebuffer row: 400 pixels padded to 32-bit alignment.
const STRIDE: usize = 52;

/// Export the Playdate entry point that runs `$game` (a `HandheldGame<Color = BinaryColor>`).
#[macro_export]
macro_rules! entry {
    ($game:ty) => {
        // The playdate-sys shim calls this symbol for every system event.
        #[allow(unsafe_code)]
        #[unsafe(no_mangle)]
        fn event_handler(
            _api: *const $crate::PlaydateAPI,
            event: $crate::PDSystemEvent,
            _arg: u32,
        ) -> $crate::EventLoopCtrl {
            $crate::run::<$game>(event)
        }

        $crate::ll_symbols!();
    };
}

/// Handle a Playdate system event: on init, create `G`, start its audio and register the
/// update callback. Called by [`entry!`].
pub fn run<G: HandheldGame<Color = BinaryColor>>(event: PDSystemEvent) -> EventLoopCtrl {
    if matches!(event, PDSystemEvent::kEventInit) {
        Accelerometer::Default().enable();
        let mut game = G::init(screen::PLAYDATE);
        if let Some(source) = game.audio() {
            start_audio(source);
        }
        System::Default().reset_elapsed_time();
        Box::leak(Box::new(Runner {
            game,
            input: Input::default(),
            clock: FrameClock::new(),
            now_us: 0,
        }))
        .set_update_handler();
    }
    EventLoopCtrl::Continue
}

struct Runner<G> {
    game: G,
    input: Input,
    clock: FrameClock,
    now_us: u64,
}

impl<G: HandheldGame<Color = BinaryColor>> Update for Runner<G> {
    fn update(&mut self) -> UpdateCtrl {
        // Menu stays with the system.
        let held = PdButtons::Default().current();
        let mut buttons = Buttons::NONE;
        buttons.set(Button::Up, held.up());
        buttons.set(Button::Down, held.down());
        buttons.set(Button::Left, held.left());
        buttons.set(Button::Right, held.right());
        buttons.set(Button::A, held.a());
        buttons.set(Button::B, held.b());

        let crank = PdCrank::Default();
        let crank = (!crank.docked()).then(|| Crank {
            angle: crank.angle(),
            delta: crank.change(),
        });

        // The Playdate's z points into the screen, the core's out of it.
        let (x, y, z) = Accelerometer::Default().get();
        let motion = Some(Motion {
            accel: [x, y, -z],
            gyro: None,
            mag: None,
        });

        self.input.advance(InputState {
            buttons,
            touch: None,
            motion,
            crank,
        });

        let system = System::Default();
        self.now_us += (system.elapsed_time_secs() * 1_000_000.0) as u64;
        system.reset_elapsed_time();
        let dt = self.clock.tick(self.now_us);
        self.game.update(&self.input, dt);

        let gfx = Graphics::Default();
        if let Ok(frame) = gfx.get_frame() {
            self.game
                .render(&mut Framebuffer1::new(frame, screen::PLAYDATE, STRIDE));
            gfx.mark_updated_rows(0, screen::PLAYDATE.height as c_int - 1);
        }
        UpdateCtrl::Continue
    }
}

/// Mix `source` into the default channel: mono, each 22.05 kHz sample played twice at 44.1 kHz.
fn start_audio<S: AudioSource>(source: S) {
    let context = Box::into_raw(Box::new(source)).cast::<c_void>();
    let callback: unsafe extern "C" fn(*mut c_void, *mut i16, *mut i16, c_int) -> c_int = fill::<S>;
    // The source plays for the rest of the program, so its handle is never removed.
    Sound::Default().add_source_raw(Some(callback), context, false);
}

extern "C" fn fill<S: AudioSource>(
    context: *mut c_void,
    left: *mut i16,
    _right: *mut i16,
    len: c_int,
) -> c_int {
    #[allow(unsafe_code)]
    // SAFETY: `context` is the `Box<S>` leaked in `start_audio` and only this callback touches
    // it; `left` holds `len` samples the mixer lends us for this call.
    let (source, out) = unsafe {
        (
            &mut *context.cast::<S>(),
            core::slice::from_raw_parts_mut(left, len.max(0) as usize),
        )
    };
    let mut half = [0i16; 128];
    for chunk in out.chunks_mut(2 * half.len()) {
        // shortcut: an odd `len` drops half a sample; the mixer asks for 256.
        let half = &mut half[..chunk.len().div_ceil(2)];
        source.fill(half);
        for (i, sample) in chunk.iter_mut().enumerate() {
            *sample = half[i / 2];
        }
    }
    1
}
