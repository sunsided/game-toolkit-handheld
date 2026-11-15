# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- `HandheldGame`: a game picks its pixel color (`Rgb565` or `BinaryColor`), reads `Input` in
  `update`, draws into any embedded-graphics `DrawTarget` in `render`, and hands over a
  `Send` `AudioSource` once.
- `Framebuffer565` and `Framebuffer1` (Playdate frame layout) draw targets, `Input` with
  button edges, touch, motion and crank, `FrameClock` for runtimes without a frame clock,
  and `screen` sizes for the Kode Dot, QVGA panels and the Playdate. `no_std`, no allocator.
- `toolkit` feature: `sim::run` plays a game in a game-toolkit window, natively or on the web;
  `HANDHELD_SHOT=out.png` saves a screenshot and quits.
- Examples: `maze` (color, sound) and `crank` (1-bit, Playdate crank).

[Unreleased]: https://github.com/sunsided/game-toolkit-handheld/commits/main
