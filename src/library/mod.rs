use std::*;
use std::borrow::Cow;
use ::audio::*;

pub mod fs;
mod release;
pub use self::release::*;


pub trait Library {
    /// Returns the unique name of this library. May not contain whitespace.
    fn name(&self) -> Cow<str>;

    // TODO: search
    fn tracks(&self) -> Result<Box<iter::Iterator<Item=Box<Track>>>, Box<error::Error>>;
}


pub enum Audio {
    Track(Box<Track>),
    Stream(Box<Stream>),
}

impl Identity for Audio {
    fn id(&self) -> (Cow<str>, Cow<str>) {
        match *self {
            Audio::Track(ref track) => track.id(),
            Audio::Stream(ref stream) => stream.id(),
        }
    }
}


pub trait Identity {
    /// Returns the library name, equal to `Library::name()` and a string that uniquely identifies
    /// an item in its library.
    fn id(&self) -> (Cow<str>, Cow<str>);
}


pub trait TrackInfo {
    fn title(&self) -> String;
    fn artists(&self) -> Vec<String>;
    /// Zero or more names of artists that have produced this track as a remix or rework of the
    /// original.
    fn remixers(&self) -> Vec<String>;
    fn genres(&self) -> Vec<String>;
    /// The title of the album if known. It is assumed that any track having the same album artists
    /// and title belongs to the same album.
    fn album_title(&self) -> Option<String>;
    fn album_artists(&self) -> Vec<String>;
    /// The disc number starting at 1 if known.
    fn album_disc(&self) -> Option<i32>;
    /// The position of this track in the album starting at 1 if known.
    fn album_track(&self) -> Option<i32>;
    /// The rating in the range of 0 to 255 inclusive if known.
    fn rating(&self) -> Option<u8>;
    fn release(&self) -> Option<Release>;
}


pub trait Track: TrackInfo + Identity {
    fn modified_at(&self) -> Option<time::SystemTime>;
    /// Constructs the audiostream for this track at the earliest available sample. This method may
    /// be called multiple times during the track's lifetime.
    fn audio(&self) -> Result<dyn::Seek, Box<error::Error>>;
    /// Returns the total duration of this track.
    fn duration(&self) -> time::Duration;
}


pub trait Stream: Identity {
    fn title(&self) -> String;
    /// Opens the stream for listening. If information about the tracks being played is available,
    /// it can be read from the returned iterator along with the time the track started playing.
    fn open(&self) -> Result<(dyn::Source, Box<iter::Iterator<Item=(Box<TrackInfo>, time::Instant)>>), Box<error::Error>>;
}


pub trait Playlist: Identity {
    fn iter(&self) -> Box<iter::Iterator<Item=Audio>>;
}
