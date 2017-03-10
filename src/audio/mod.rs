use std::*;
use sample;

pub mod dyn;


pub trait Source: iter::Iterator
    where Self::Item: sample::Frame {
    /// Returns the number of frames per second that should be read to assure realtime playback.
    /// An implementation may not dynamically change its value.
    fn sample_rate(&self) -> u32;
}

/// When a Source has a known finite number of frames, it may implement the Seek trait.
pub trait Seek: Source
    where Self::Item: sample::Frame {
    /// Seeks to the frame with the position specified by pos. The proceeding calls to next()
    /// should yield the frame at the specified index.
    fn seek(&mut self, pos: io::SeekFrom) -> Result<(), Box<error::Error>>;
    /// Returns the total number of frames in the stream.
    fn length(&self) -> u64;
    /// Retrieves the index of the frame that will be read next.
    fn position(&self) -> u64;
    /// Calculates the total duration of the signal based on the sample rate and number of samples.
    fn duration(&self) -> time::Duration {
        time::Duration::new(self.length() / self.sample_rate() as u64, 0)
    }
}

pub trait Sink<F>
    where F: sample::Frame {
    fn write_frame(&mut self, frame: F) -> Result<(), Box<error::Error>>;
    /// See `Source::sample_rate`.
    fn sample_rate(&self) -> u32;
}
