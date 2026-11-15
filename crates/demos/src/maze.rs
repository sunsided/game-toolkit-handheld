//! A color D-pad maze: walk the square to the goal, it beeps and the maze flips.
//!
//! Shows an `Rgb565` game and sound split from the game: the audio source runs wherever the
//! backend mixes (a thread here, an I2S DMA task on a board) and hears about the goal through
//! an atomic, not a shared borrow of the game.

use core::convert::Infallible;
use core::sync::atomic::{AtomicU32, Ordering};

use game_toolkit_handheld::{
    AUDIO_RATE, AudioSource, Button, HandheldGame, Input,
    embedded_graphics::{
        pixelcolor::Rgb565,
        prelude::*,
        primitives::{PrimitiveStyle, Rectangle},
    },
};

const MAZE: [&str; 7] = [
    "###########",
    "#.....#...#",
    "#.###.#.#.#",
    "#.#...#.#.#",
    "#.#.###.#.#",
    "#...#.....G",
    "###########",
];

/// Samples of beep still to play; written by the game, counted down by the audio source.
static BEEP: AtomicU32 = AtomicU32::new(0);

/// The color D-pad maze.
pub struct Maze {
    cell: u32,
    origin: Point,
    player: Point,
    flipped: bool,
}

impl Maze {
    fn wall(&self, p: Point) -> bool {
        let row = MAZE[p.y as usize].as_bytes();
        let x = if self.flipped {
            row.len() - 1 - p.x as usize
        } else {
            p.x as usize
        };
        row[x] == b'#'
    }

    fn goal(&self, p: Point) -> bool {
        let x = if self.flipped {
            MAZE[0].len() as i32 - 1 - p.x
        } else {
            p.x
        };
        MAZE[p.y as usize].as_bytes()[x as usize] == b'G'
    }
}

impl HandheldGame for Maze {
    type Color = Rgb565;

    fn init(screen: Size) -> Self {
        let cols = MAZE[0].len() as u32;
        let rows = MAZE.len() as u32;
        let cell = (screen.width / cols).min(screen.height / rows);
        let origin = Point::new(
            ((screen.width - cell * cols) / 2) as i32,
            ((screen.height - cell * rows) / 2) as i32,
        );
        Self {
            cell,
            origin,
            player: Point::new(1, 1),
            flipped: false,
        }
    }

    fn update(&mut self, input: &Input, _dt: f32) {
        let step = [
            (Button::Up, Point::new(0, -1)),
            (Button::Down, Point::new(0, 1)),
            (Button::Left, Point::new(-1, 0)),
            (Button::Right, Point::new(1, 0)),
        ]
        .into_iter()
        .find(|&(b, _)| input.pressed(b));
        if let Some((_, d)) = step {
            let next = self.player + d;
            if !self.wall(next) {
                self.player = next;
            }
            if self.goal(self.player) {
                BEEP.store(AUDIO_RATE / 8, Ordering::Relaxed);
                self.flipped = !self.flipped;
                self.player = Point::new(
                    if self.flipped {
                        MAZE[0].len() as i32 - 2
                    } else {
                        1
                    },
                    1,
                );
            }
        }
    }

    fn render<D: DrawTarget<Color = Rgb565, Error = Infallible>>(&mut self, target: &mut D) {
        let Ok(()) = target.clear(Rgb565::new(2, 4, 6));
        let cell = Size::new_equal(self.cell);
        let at =
            |p: Point| self.origin + Point::new(p.x * self.cell as i32, p.y * self.cell as i32);
        for (y, row) in MAZE.iter().enumerate() {
            for x in 0..row.len() {
                let p = Point::new(x as i32, y as i32);
                let color = if self.wall(p) {
                    Rgb565::new(8, 20, 24)
                } else if self.goal(p) {
                    Rgb565::YELLOW
                } else {
                    continue;
                };
                let Ok(()) = Rectangle::new(at(p), cell)
                    .into_styled(PrimitiveStyle::with_fill(color))
                    .draw(target);
            }
        }
        let inset = (self.cell / 6) as i32;
        let Ok(()) = Rectangle::new(
            at(self.player) + Point::new_equal(inset),
            cell - Size::new_equal(2 * inset as u32),
        )
        .into_styled(PrimitiveStyle::with_fill(Rgb565::MAGENTA))
        .draw(target);
    }

    fn audio(&mut self) -> Option<impl AudioSource> {
        let mut phase = 0u32;
        Some(move |out: &mut [i16]| {
            for s in out {
                let left = BEEP.load(Ordering::Relaxed);
                if left == 0 {
                    *s = 0;
                    continue;
                }
                BEEP.store(left - 1, Ordering::Relaxed);
                // 880 Hz square wave.
                phase = (phase + 880) % AUDIO_RATE;
                *s = if phase < AUDIO_RATE / 2 { 6000 } else { -6000 };
            }
        })
    }
}
