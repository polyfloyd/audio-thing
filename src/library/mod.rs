use std::*;
use std::borrow::Cow;
use std::sync::{Arc, Mutex};
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


pub trait Identity: Send + Sync {
    /// Returns the library name, equal to `Library::name()` and a string that uniquely identifies
    /// an item in its library.
    fn id(&self) -> (Cow<str>, Cow<str>);
}

impl<'a> Identity for &'a Identity {
    fn id(&self) -> (Cow<str>, Cow<str>) { (*self).id() }
}


pub trait TrackInfo {
    fn title(&self) -> Cow<str>;
    fn artists(&self) -> Cow<[String]>;
    /// Zero or more names of artists that have produced this track as a remix or rework of the
    /// original.
    fn remixers(&self) -> Cow<[String]>;
    fn genres(&self) -> Cow<[String]>;
    /// The title of the album if known. It is assumed that any track having the same album artists
    /// and title belongs to the same album.
    fn album_title(&self) -> Option<Cow<str>>;
    fn album_artists(&self) -> Cow<[String]>;
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
    fn title(&self) -> Cow<str>;
    /// Opens the stream for listening. If information about the tracks being played is available,
    /// it can be read from the returned iterator along with the time the track started playing.
    fn open(&self) -> Result<(dyn::Source, Box<iter::Iterator<Item=(Box<TrackInfo>, time::Instant)>>), Box<error::Error>>;
}


pub trait Playlist: Identity {
    /// Gets the number of tracks in the playlist without loading its complete contents.
    fn len(&self) -> Result<usize, Box<error::Error>>;
    /// TODO: Cow<[Audio]>?
    fn contents(&self) -> Result<Vec<Audio>, Box<error::Error>>;

    /// It is possible that a playlist can not be modified, e.g. a read-only HTTP API. Because this
    /// is often known in advance, the playlist should be upgraded into a mutable playlist with
    /// which mutations can be performed.
    fn as_mut(&mut self) -> Option<&mut PlaylistMut>;
}

pub trait PlaylistMut: Playlist {
    /// Replaces the entire playlist with another.
    fn set_contents(&mut self, &[&Identity]) -> Result<(), Box<error::Error>>;

    /// Inserts the given audio into the specified position.
    fn insert(&mut self, position: usize, audio: &[&Identity]) -> Result<(), Box<error::Error>> {
        let orig = self.contents()?;
        let contents: Vec<&Identity> = orig[..position].iter().map(|r| -> &Identity { r })
            .chain(audio.into_iter().map(|r| -> &Identity { r }))
            .chain(orig[position..].iter().map(|r| -> &Identity { r }))
            .collect();
        self.set_contents(contents.as_slice())?;
        Ok(())
    }

    /// Removes the specified range from the playlist.
    fn remove(&mut self, range: ops::Range<usize>) -> Result<(), Box<error::Error>> {
        let orig = self.contents()?;
        let contents: Vec<&Identity> = orig.iter().take(range.start)
            .chain(orig.iter().skip(range.end))
            .map(|r| -> &Identity { r })
            .collect();
        self.set_contents(contents.as_slice())?;
        Ok(())
    }

    /// Moves one or more elements to another position. The insert position must be smaller than
    /// the length of the playlist. The `to` position is relative to the current state of the
    /// playlist.
    ///
    /// If `to` is inside the range to be moved, this is a no-op.
    fn splice(&mut self, from: ops::Range<usize>, to: usize) -> Result<(), Box<error::Error>> {
        let orig = self.contents()?;
        let contents: Vec<&Identity> =
            if to < from.start {
                orig[0..from.start].iter()
                    .chain(orig[from.end..to].iter())
                    .chain(orig[from.start..from.end].iter())
                    .chain(orig[to..].iter())
            } else if to >= from.end {
                orig[0..to].iter()
                    .chain(orig[from.start..from.end].iter())
                    .chain(orig[to..from.start].iter())
                    .chain(orig[from.end..].iter())
            } else {
                // Target position is inside the range to be moved.
                return Ok(());
            }
            .map(|r| -> &Identity { r })
            .collect();
        self.set_contents(contents.as_slice())?;
        Ok(())
    }

    /// Reorders all items of this playlist based on the indices specified. The list index is the
    /// new position of the element while the value is the current position.
    ///
    /// The index list should have the same size as this playlist.
    fn move_all(&mut self, from: &[usize]) -> Result<(), Box<error::Error>> {
        let orig = self.contents()?;
        if from.len() != orig.len() {
            unimplemented!();
        }
        let contents: Vec<&Identity> = (0..orig.len())
            .map(|i| -> &Identity { &orig[from[i]] })
            .collect();
        self.set_contents(contents.as_slice())?;
        Ok(())
    }
}
