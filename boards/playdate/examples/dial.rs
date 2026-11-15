//! The crank dial demo on the Playdate.
#![no_std]
// The device build links this as a bin whose entry is the SDK's `eventHandlerShim`.
#![cfg_attr(target_os = "none", no_main)]

board_playdate::entry!(game_toolkit_handheld_demos::Dial);
