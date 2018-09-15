use audio::*;
use rand::{self, Rng};
use std::borrow::Cow;
use std::collections::BTreeSet;
use std::sync::Arc;
use std::*;

pub mod fs;
mod release;
pub use self::release::*;

pub trait Library: Send + Sync {
    /// Returns the unique name of this library. May not contain whitespace.
    fn name(&self) -> Cow<str>;

    fn find_by_id(&self, id: &Identity) -> Result<Option<Audio>, Box<error::Error>>;

    // TODO: search
    fn tracks(&self) -> Result<Box<iter::Iterator<Item = Arc<Track>>>, Box<error::Error>>;
}

pub fn resolve_all<L>(libs: &[L], ids: &[&Identity]) -> Result<Vec<Audio>, Box<error::Error>>
where
    L: borrow::Borrow<Library>,
{
    ids.into_iter()
        .filter_map(|id| {
            let (name, _) = id.id();
            let lib = libs.iter().find(|lib| lib.borrow().name() == name);
            let lib = match lib {
                Some(lib) => lib,
                None => {
                    return Some(Err(Box::from(PlaylistError::MissingLibrary(
                        name.into_owned(),
                    ))))
                }
            };
            match lib.borrow().find_by_id(id) {
                Ok(Some(audio)) => Some(Ok(audio)),
                Ok(None) => None,
                Err(err) => Some(Err(err)),
            }
        }).collect()
}

#[derive(Clone)]
pub enum Audio {
    Track(Arc<Track>),
    Stream(Arc<Stream>),
}

impl Audio {
    pub fn track(&self) -> Option<&Track> {
        match *self {
            Audio::Track(ref track) => Some(track.as_ref()),
            Audio::Stream(_) => None,
        }
    }
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
    fn id(&self) -> (Cow<str>, Cow<str>) {
        (*self).id()
    }
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
    /// The rating (number of stars) ranging from 1 to 5 inclusive if known.
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
    /// it is applied to the specified callback.
    fn open(
        &self,
        on_info: Arc<Fn(Option<Box<TrackInfo + Send>>)>,
    ) -> Result<dyn::Source, Box<error::Error>>;
}

pub trait Playlist {
    /// Gets the number of tracks in the playlist without loading its complete contents.
    fn len(&self) -> Result<usize, Box<error::Error>>;
    /// Returns the contents of the playlist
    fn contents(&self) -> Result<Cow<[Audio]>, Box<error::Error>>;

    /// It is possible that a playlist can not be modified, e.g. a read-only HTTP API. Because this
    /// is often known in advance, the playlist should be upgraded into a mutable playlist with
    /// which mutations can be performed.
    fn as_mut(&mut self) -> Option<&mut PlaylistMut>;
}

pub trait PlaylistMut: Playlist {
    /// Replaces the entire playlist with another.
    fn set_contents(&mut self, new: &[&Identity]) -> Result<(), Box<error::Error>>;

    /// Inserts the given audio into the specified position.
    fn insert(&mut self, position: usize, audio: &[&Identity]) -> Result<(), Box<error::Error>> {
        let orig = self.contents()?.into_owned();
        let contents: Vec<&Identity> = orig[..position]
            .iter()
            .map(|r| r as &Identity)
            .chain(audio.into_iter().map(|r| -> &Identity { r }))
            .chain(orig[position..].iter().map(|r| r as &Identity))
            .collect();
        self.set_contents(contents.as_slice())?;
        Ok(())
    }

    /// Removes the specified range from the playlist.
    ///
    /// An error is returned if the range exceeds the size of the playlist.
    fn remove(&mut self, range: ops::Range<usize>) -> Result<(), Box<error::Error>> {
        let orig = self.contents()?.into_owned();
        if range.end >= orig.len() {
            return Err(Box::from(PlaylistError::IndexOutOfBounds));
        }
        let contents: Vec<&Identity> = orig
            .iter()
            .take(range.start)
            .chain(orig.iter().skip(range.end))
            .map(|r| r as &Identity)
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
        let orig: Vec<_> = (0..self.len()?).collect();
        let contents: Vec<_> = if to < from.start {
            orig[0..from.start]
                .iter()
                .chain(orig[from.end..to].iter())
                .chain(orig[from.start..from.end].iter())
                .chain(orig[to..].iter())
        } else if to >= from.end {
            orig[0..to]
                .iter()
                .chain(orig[from.start..from.end].iter())
                .chain(orig[to..from.start].iter())
                .chain(orig[from.end..].iter())
        } else {
            // Target position is inside the range to be moved.
            return Ok(());
        }.cloned()
        .collect();
        self.move_all(contents.as_slice())?;
        Ok(())
    }

    /// Reorders all items of this playlist based on the indices specified. The list index is the
    /// new position of the element while the value is the current position.
    ///
    /// Returns an error if either:
    /// * The index list should have the same size as this playlist.
    /// * One or more indices are out of bounds.
    /// * There are duplicate indices.
    fn move_all(&mut self, from: &[usize]) -> Result<(), Box<error::Error>> {
        let orig = self.contents()?.into_owned();
        if from.len() != orig.len() {
            return Err(Box::from(PlaylistError::MoveLengthMismatch));
        }
        let contents = (0..orig.len())
            .map(|i| -> Result<&Identity, _> {
                if from[i] >= orig.len() {
                    return Err(PlaylistError::IndexOutOfBounds);
                }
                Ok(&orig[from[i]])
            }).collect::<Result<Vec<_>, _>>();
        let contents = contents?;
        if contents.len() != orig.len() {
            return Err(Box::from(PlaylistError::MoveDuplicateIndices));
        }
        self.set_contents(contents.as_slice())?;
        Ok(())
    }

    /// Randomly reorders the contents of the playlist.
    fn shuffle(&mut self) -> Result<(), Box<error::Error>> {
        let mut rng = rand::thread_rng();
        let mut set: BTreeSet<usize> = (0..self.len()?).collect();
        let mut shuffled = Vec::with_capacity(set.len());
        for _ in 0..set.len() {
            let v = rng.gen::<usize>() % set.len();
            shuffled.push(set.take(&v).unwrap())
        }
        self.move_all(shuffled.as_slice())?;
        Ok(())
    }
}

#[derive(Debug)]
pub enum PlaylistError {
    MissingLibrary(String),
    IndexOutOfBounds,
    MoveLengthMismatch,
    MoveDuplicateIndices,
}

impl fmt::Display for PlaylistError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            PlaylistError::MissingLibrary(ref name) => write!(f, "No library named {}", name),
            PlaylistError::IndexOutOfBounds => {
                write!(f, "An index is larger than the playlist size")
            }
            PlaylistError::MoveLengthMismatch => write!(
                f,
                "Unable to move all elements, length of argument array mismatched"
            ),
            PlaylistError::MoveDuplicateIndices => write!(
                f,
                "Unable to move all elements, there are duplicate indices"
            ),
        }
    }
}

impl error::Error for PlaylistError {
    fn description(&self) -> &str {
        "Playlist error"
    }

    fn cause(&self) -> Option<&error::Error> {
        None
    }
}
