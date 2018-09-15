use audio::*;
use library;
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, Weak};
use std::*;

pub mod output;
pub mod playback;
pub use self::playback::*;

/// A player manages the playback of audio from a list of audio. Multple tracks can be played at
/// once to make mixing and crossfading possible.
///
/// On-the-fly modifications, like the playstate, tempo and position, can be made to individual
/// playing tracks through the `playing` map.
/// Playback of all tracks can also be managed through the player itself. For this, the player The
/// player has one master and zero or more slave tracks.
pub struct Player {
    /// The tracks that are currently playing. When a track finishes or the playback is stopped
    /// manually, it will be removed from the map.
    pub playing: BTreeMap<
        u64,
        (
            library::Audio,
            Playback,
            Option<Box<library::TrackInfo + Send>>,
        ),
    >,
    /// Used for generating the next playback key.
    gen_next_id: u64,

    output: Box<output::Output + Send>,

    pub queue: Vec<library::Audio>,
    pub queue_autofill: Box<iter::Iterator<Item = library::Audio> + Send>,
    pub queue_cursor: Option<usize>,

    pub libraries: Vec<Arc<library::Library>>,

    /// A weak reference to this player to be used in event handlers.
    weak_self: Weak<Mutex<Player>>,
}

impl Player {
    pub fn new(
        output: Box<output::Output + Send>,
        libraries: Vec<Arc<library::Library>>,
    ) -> Arc<Mutex<Player>> {
        let p = Arc::new(Mutex::new(Player {
            playing: BTreeMap::new(),
            gen_next_id: 0,
            output,
            queue: Vec::new(),
            queue_cursor: None,
            queue_autofill: Box::from(iter::empty()),
            libraries,
            weak_self: Weak::new(),
        }));
        p.lock().unwrap().weak_self = Arc::downgrade(&p);
        p
    }

    /// Sets up playback for the specified track. The initial state is set to paused.
    fn init_playback(&mut self, audio: &library::Audio) -> Result<(u64, &mut Playback), Error> {
        self.gen_next_id += 1;
        let id = self.gen_next_id;

        let weak = self.weak_self.clone();
        let signal: dyn::Audio = match audio {
            library::Audio::Track(ref track) => track.audio()?.into(),
            library::Audio::Stream(ref stream) => {
                let on_info = Arc::new(move |info| {
                    let arc = match weak.upgrade() {
                        Some(arc) => arc,
                        None => return,
                    };
                    thread::spawn(move || {
                        let mut player = arc.lock().unwrap();
                        if let Some(&mut (_, _, ref mut i)) = player.playing.get_mut(&id) {
                            *i = info;
                        }
                    });
                });
                stream.open(on_info)?.into()
            }
        };

        let weak = self.weak_self.clone();
        let playback = Playback::new(
            signal,
            &*self.output,
            Arc::new(move |event| {
                let arc = match weak.upgrade() {
                    Some(arc) => arc,
                    None => return,
                };
                // Because mutations performed on the player may fire event that will in turn mutate
                // the player, the handler is run asynchronously to prevent deadlocks.
                thread::spawn(move || {
                    let mut player = arc.lock().unwrap();
                    match event {
                        playback::Event::Output(output::Event::End) => {
                            // Only advance the queue cursor if the track naturally ended.
                            if let Err(err) = player.play_next_from_queue() {
                                error!("{}", err);
                                // TODO: (player.event_handers)(Event::Error(err));
                            }
                        }
                        playback::Event::Output(output::Event::Error(err)) => {
                            error!("{}", err);
                        }
                        playback::Event::State(state) => {
                            // GC tracks that have been stopped.
                            if state == State::Stopped {
                                player.playing.remove(&id);
                            }
                        }
                        _ => (),
                    }
                });
            }),
        );
        self.playing.insert(id, (audio.clone(), playback, None));
        Ok((id, &mut self.playing.get_mut(&id).unwrap().1))
    }

    pub fn play_from_queue(&mut self, index: usize) -> Result<Option<(u64, &mut Playback)>, Error> {
        let audio = match self.queue.get(index) {
            Some(audio) => audio.clone(),
            None => return Ok(None),
        };
        self.playing.clear();
        self.queue_cursor = Some(index);
        let (id, pb) = self.init_playback(&audio)?;
        pb.set_state(State::Playing);
        Ok(Some((id, pb)))
    }

    pub fn play_previous_from_queue(&mut self) -> Result<Option<(u64, &mut Playback)>, Error> {
        let index = self
            .queue_cursor
            .and_then(|i| i.checked_sub(1))
            .unwrap_or(0);
        self.play_from_queue(index)
    }

    /// Stops all currently playing tracks, advances the queue cursor and starts playing the track
    /// at that position.
    ///
    /// If the cursor has reached the end of the queue, a track from the queue autofill, if any, is
    /// appended and played.
    pub fn play_next_from_queue(&mut self) -> Result<Option<(u64, &mut Playback)>, Error> {
        let index = self.queue_cursor.map(|i| i + 1).unwrap_or(0);
        if index >= self.queue.len() {
            self.queue.extend(self.queue_autofill.next().into_iter());
        }
        self.play_from_queue(index)
    }
}

impl library::Playlist for Player {
    fn len(&self) -> Result<usize, Box<error::Error>> {
        Ok(self.queue.len())
    }

    fn contents(&self) -> Result<Cow<[library::Audio]>, Box<error::Error>> {
        Ok(Cow::Borrowed(&self.queue[..]))
    }

    fn as_mut(&mut self) -> Option<&mut library::PlaylistMut> {
        Some(self)
    }
}

impl library::PlaylistMut for Player {
    fn set_contents(&mut self, new: &[&library::Identity]) -> Result<(), Box<error::Error>> {
        self.queue = library::resolve_all(&self.libraries[..], new)?;
        self.queue_cursor = None;
        Ok(())
    }

    fn insert(
        &mut self,
        position: usize,
        audio: &[&library::Identity],
    ) -> Result<(), Box<error::Error>> {
        if position > self.queue.len() {
            return Err(Box::from(library::PlaylistError::IndexOutOfBounds));
        }
        let resolved = library::resolve_all(&self.libraries[..], audio)?;
        if let Some(cur) = self.queue_cursor.as_mut() {
            if position < *cur {
                *cur += audio.len();
            }
        }
        let tail = self.queue.split_off(position);
        self.queue.extend(resolved);
        self.queue.extend(tail);
        Ok(())
    }

    fn remove(&mut self, range: ops::Range<usize>) -> Result<(), Box<error::Error>> {
        if range.end >= self.queue.len() {
            return Err(Box::from(library::PlaylistError::IndexOutOfBounds));
        }
        if let Some(cur) = self.queue_cursor.as_mut() {
            if *cur >= range.end {
                *cur -= range.len();
            } else if range.start <= *cur && range.end < *cur {
                *cur = range.start;
            }
        }
        self.queue.drain(range);
        Ok(())
    }

    fn move_all(&mut self, from: &[usize]) -> Result<(), Box<error::Error>> {
        if from.len() != self.queue.len() {
            return Err(Box::from(library::PlaylistError::MoveLengthMismatch));
        }
        let new_queue = (0..from.len())
            .map(|i| {
                self.queue
                    .get(i)
                    .cloned()
                    .ok_or(library::PlaylistError::IndexOutOfBounds)
            }).collect::<Result<Vec<_>, _>>()?;
        if self.queue.len() != new_queue.len() {
            return Err(Box::from(library::PlaylistError::MoveDuplicateIndices));
        }

        if let Some(cur) = self.queue_cursor.as_mut() {
            *cur = *from.iter().find(|i| **i == *cur).unwrap();
        }
        self.queue = new_queue;
        Ok(())
    }
}

#[derive(Debug)]
pub enum Error {
    Other(Box<error::Error>),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::Other(ref err) => err.fmt(f),
        }
    }
}

impl error::Error for Error {
    fn description(&self) -> &str {
        "Player error"
    }

    fn cause(&self) -> Option<&error::Error> {
        None
    }
}

impl From<Box<error::Error>> for Error {
    fn from(err: Box<error::Error>) -> Error {
        Error::Other(err)
    }
}
