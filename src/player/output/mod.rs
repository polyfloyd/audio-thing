use std::*;
use ::audio::*;

pub mod pulse;


pub trait Output {
    /// Starts playing audio from the specified source.
    /// This method may be called multiple times during the lifetime of the output to allow playing
    /// and mixing multiple streams at once.
    /// The implementation should take into account that the source may block at any given moment.
    fn consume(&self, source: dyn::Source) -> Result<Box<Stream>, Box<error::Error>>;
}

pub trait Stream {
    /// Returns the volume set by `set_volume()`. The initial value is undefined.
    fn volume(&self) -> Result<f64, Box<error::Error>>;
    /// Sets the volume of the output using a hardware if available. A software mixer may be used
    /// to provide this functionality if no hardware mixer is available.
    /// Values range from 0.0 to 1.0 inclusive.
    fn set_volume(&mut self, volume: f64) -> Result<(), Box<error::Error>>;

    /// Returns the approximate latency of the audio output or 0 if it can not be reliably
    /// determined.
    fn latency(&self) -> Result<time::Duration, Box<error::Error>>;
}
