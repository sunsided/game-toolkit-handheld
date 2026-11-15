//! The color maze in the simulator: arrows/WASD move, Esc quits.

use game_toolkit_handheld::{
    screen,
    sim::{self, SimConfig},
};
use game_toolkit_handheld_demos::Maze;

fn main() -> game_toolkit::prelude::Result<()> {
    env_logger::init();
    sim::run::<Maze>(SimConfig::new("maze", screen::QVGA))
}
