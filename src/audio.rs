use std::*;
use sample;

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
    /// Returns the total number of frames in the stream
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

pub mod dyn {
    pub enum Source {
        MonoI8(   Box<super::Source<Item=[i8;  1]>>),
        MonoU8(   Box<super::Source<Item=[u8;  1]>>),
        MonoI16(  Box<super::Source<Item=[i16; 1]>>),
        MonoU16(  Box<super::Source<Item=[u16; 1]>>),
        MonoI32(  Box<super::Source<Item=[i32; 1]>>),
        MonoU32(  Box<super::Source<Item=[u32; 1]>>),
        MonoI64(  Box<super::Source<Item=[i64; 1]>>),
        MonoU64(  Box<super::Source<Item=[u64; 1]>>),
        MonoF32(  Box<super::Source<Item=[f32; 1]>>),
        MonoF64(  Box<super::Source<Item=[f64; 1]>>),
        StereoI8( Box<super::Source<Item=[i8;  2]>>),
        StereoU8( Box<super::Source<Item=[u8;  2]>>),
        StereoI16(Box<super::Source<Item=[i16; 2]>>),
        StereoU16(Box<super::Source<Item=[u16; 2]>>),
        StereoI32(Box<super::Source<Item=[i32; 2]>>),
        StereoU32(Box<super::Source<Item=[u32; 2]>>),
        StereoI64(Box<super::Source<Item=[i64; 2]>>),
        StereoU64(Box<super::Source<Item=[u64; 2]>>),
        StereoF32(Box<super::Source<Item=[f32; 2]>>),
        StereoF64(Box<super::Source<Item=[f64; 2]>>),
    }

    pub enum Seek {
        MonoI8(   Box<super::Seek<Item=[i8;  1]>>),
        MonoU8(   Box<super::Seek<Item=[u8;  1]>>),
        MonoI16(  Box<super::Seek<Item=[i16; 1]>>),
        MonoU16(  Box<super::Seek<Item=[u16; 1]>>),
        MonoI32(  Box<super::Seek<Item=[i32; 1]>>),
        MonoU32(  Box<super::Seek<Item=[u32; 1]>>),
        MonoI64(  Box<super::Seek<Item=[i64; 1]>>),
        MonoU64(  Box<super::Seek<Item=[u64; 1]>>),
        MonoF32(  Box<super::Seek<Item=[f32; 1]>>),
        MonoF64(  Box<super::Seek<Item=[f64; 1]>>),
        StereoI8( Box<super::Seek<Item=[i8;  2]>>),
        StereoU8( Box<super::Seek<Item=[u8;  2]>>),
        StereoI16(Box<super::Seek<Item=[i16; 2]>>),
        StereoU16(Box<super::Seek<Item=[u16; 2]>>),
        StereoI32(Box<super::Seek<Item=[i32; 2]>>),
        StereoU32(Box<super::Seek<Item=[u32; 2]>>),
        StereoI64(Box<super::Seek<Item=[i64; 2]>>),
        StereoU64(Box<super::Seek<Item=[u64; 2]>>),
        StereoF32(Box<super::Seek<Item=[f32; 2]>>),
        StereoF64(Box<super::Seek<Item=[f64; 2]>>),
    }
}
