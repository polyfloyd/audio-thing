use std::*;
use std::ops::{Deref, DerefMut};
use sample;

pub mod dyn;


pub trait Source: iter::Iterator
    where Self::Item: sample::Frame {
    /// Returns the number of frames per second that should be read to assure realtime playback.
    /// An implementation may not dynamically change its value.
    fn sample_rate(&self) -> u32;
}

/// Seekable provides the methods that allow seeking in an audio source. The distinction between
/// `Seekable` and `Seek` is to allow seeking without templating by taking a reference from a
/// `Seek`. Prefer `Seek` if possible.
pub trait Seekable {
    /// Seeks to the frame at the specified position. The proceeding calls to next()
    /// should yield the frame at that position.
    fn seek(&mut self, position: u64) -> Result<(), SeekError>;
    /// Returns the total number of frames in the stream.
    fn length(&self) -> u64;
    /// Retrieves the index of the frame that will be read next.
    fn current_position(&self) -> u64;
}

/// When a Source has a known finite number of frames, it may implement the Seek trait to allow
/// random access.
pub trait Seek: Source + Seekable
    where Self::Item: sample::Frame {
}


#[derive(Debug)]
pub enum SeekError {
    Other(Box<error::Error>),
    OutofRange{
        pos: u64,
        size: u64,
    },
}

impl fmt::Display for SeekError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            SeekError::Other(ref err) => {
                write!(f, "Error seeking: {}", err)
            },
            SeekError::OutofRange{pos, size} => {
                write!(f, "Error seeking: index out of range: {} (size: {})", pos, size)
            },
        }
    }
}

impl error::Error for SeekError {
    fn description(&self) -> &str {
        "Seek error"
    }

    fn cause(&self) -> Option<&error::Error> {
        match *self {
            SeekError::Other(ref err) => Some(err.deref()),
            _ => None,
        }
    }
}


pub fn duration_of(sample_rate: u32, num_samples: u64) -> time::Duration {
    let secs = num_samples / sample_rate as u64;
    let nanos = (num_samples as u32 % sample_rate) * (1_000_000_000 / sample_rate);
    time::Duration::new(secs, nanos)
}


pub trait Sink<F>
    where F: sample::Frame {
    fn write_frame(&mut self, frame: F) -> Result<(), Box<error::Error + Send>>;
    /// See `Source::sample_rate`.
    fn sample_rate(&self) -> u32;
}


pub struct FromIter<S>
    where S: iter::Iterator,
          S::Item: sample::Frame {
    signal: S,
    sample_rate: u32
}

impl<S> iter::Iterator for FromIter<S>
    where S: iter::Iterator,
          S::Item: sample::Frame {
    type Item = S::Item;
    fn next(&mut self) -> Option<Self::Item> { self.signal.next() }
}

impl<S> Source for FromIter<S>
    where S: iter::Iterator,
          S::Item: sample::Frame {
    fn sample_rate(&self) -> u32 { self.sample_rate }
}

pub trait IntoSource: iter::Iterator + Sized
    where Self::Item: sample::Frame {
    fn source(self, sample_rate: u32) -> FromIter<Self> {
        FromIter{ signal: self, sample_rate: sample_rate }
    }
}

impl<T> IntoSource for T
    where T: iter::Iterator,
          T::Item: sample::Frame { }


/// This type allows a source to be shared between multiple threads. This is especially usefull for
/// DSP nodes that allow modification of some parameters while it is being consumed.
///
/// It's important to note that each call to a member function will involve locking it's associated
/// mutex. Therefore, if the underlying source blocks, so will all concurrent operations.
#[derive(Clone)]
pub struct Shared<S>
    where S: Source,
          S::Item: sample::Frame {
    pub input: sync::Arc<sync::Mutex<S>>,
}

impl<S> iter::Iterator for Shared<S>
    where S: Source,
          S::Item: sample::Frame {
    type Item = S::Item;
    fn next(&mut self) -> Option<Self::Item> {
        self.input.lock().unwrap().next()
    }
}

impl<S> Source for Shared<S>
    where S: Source,
          S::Item: sample::Frame {
    fn sample_rate(&self) -> u32 {
        self.input.lock().unwrap().sample_rate()
    }
}

impl<S> Seekable for Shared<S>
    where S: Source + Seekable,
          S::Item: sample::Frame {
    fn seek(&mut self, position: u64) -> Result<(), SeekError> {
        self.input.lock().unwrap().seek(position)
    }

    fn length(&self) -> u64 {
        self.input.lock().unwrap().length()
    }

    fn current_position(&self) -> u64 {
        self.input.lock().unwrap().current_position()
    }
}

pub trait IntoShared: Source + Sized
    where Self::Item: sample::Frame {
    fn shared(self) -> Shared<Self> {
        Shared{ input: sync::Arc::new(sync::Mutex::new(self)) }
    }
}

impl<T> IntoShared for T
    where T: Source,
          T::Item: sample::Frame { }


impl<T> Source for Box<T>
    where T: Source + ?Sized,
          T::Item: sample::Frame {
    fn sample_rate(&self) -> u32 {
        self.deref().sample_rate()
    }
}

impl<T> Seekable for Box<T>
    where T: Seekable + ?Sized {
    fn seek(&mut self, position: u64) -> Result<(), SeekError> {
        self.deref_mut().seek(position)
    }

    fn length(&self) -> u64 {
        self.deref().length()
    }

    fn current_position(&self) -> u64 {
        self.deref().current_position()
    }
}

impl<T> Seek for Box<T>
    where T: Seek + ?Sized,
          T::Item: sample::Frame { }
