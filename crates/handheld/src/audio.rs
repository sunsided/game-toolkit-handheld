/// Sample rate of every [`AudioSource`], in Hz. Backends whose hardware runs faster repeat
/// samples (the Playdate mixes at 44.1 kHz, exactly twice this).
pub const AUDIO_RATE: u32 = 22_050;

/// Mono 16-bit PCM at [`AUDIO_RATE`], pulled by the backend's audio interrupt, DMA task,
/// callback or thread.
///
/// `fill` may run on another thread or in interrupt context, so keep it short, don't block
/// and don't allocate. Talk to the game through a lock-free queue or atomics the game sets up
/// before handing the source over. Any `FnMut(&mut [i16]) + Send + 'static` closure is a source.
pub trait AudioSource: Send + 'static {
    /// Overwrite `out` with the next samples.
    fn fill(&mut self, out: &mut [i16]);
}

impl<F: FnMut(&mut [i16]) + Send + 'static> AudioSource for F {
    fn fill(&mut self, out: &mut [i16]) {
        self(out);
    }
}
