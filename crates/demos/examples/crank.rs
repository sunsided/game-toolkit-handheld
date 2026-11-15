//! The 1-bit crank dial in the simulator: mouse wheel or Q/E turns, X inverts.

use game_toolkit_handheld::{
    screen,
    sim::{self, SimConfig},
};
use game_toolkit_handheld_demos::Dial;

fn main() -> game_toolkit::prelude::Result<()> {
    env_logger::init();
    sim::run::<Dial>(SimConfig::new("crank", screen::PLAYDATE))
}
