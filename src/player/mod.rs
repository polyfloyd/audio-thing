use std::*;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, Weak};
use ::audio::*;
use ::library;

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
    pub playing: BTreeMap<u64, (Arc<library::Audio>, Playback)>,

    /// Used for generating the next playback key.
    gen_next_id: u64,

    output: Box<output::Output + Send>,

    pub queue: Vec<Arc<library::Audio>>,
    pub queue_autofill: Box<iter::Iterator<Item=library::Audio> + Send>,
    queue_cursor: Option<usize>,

    /// A weak reference to this player to be used in event handlers.
    weak_self: Weak<Mutex<Player>>,
}

impl Player {
    pub fn new(output: Box<output::Output + Send>) -> Arc<Mutex<Player>> {
        let p = Arc::new(Mutex::new(Player {
            playing: BTreeMap::new(),
            gen_next_id: 0,
            output: output,
            queue: Vec::new(),
            queue_cursor: None,
            queue_autofill: Box::from(iter::empty()),
            weak_self: Weak::new(),
        }));
        p.lock().unwrap().weak_self = Arc::downgrade(&p);
        p
    }

    /// Sets up playback for the specified track. The initial state is set to paused.
    fn init_playback(&mut self, audio: Arc<library::Audio>) -> Result<(u64, &mut Playback), Error> {
        let signal: dyn::Audio = match audio.as_ref() {
            &library::Audio::Track(ref track) => track.audio()?.into(),
            &library::Audio::Stream(_) => unimplemented!(),
        };
        self.gen_next_id += 1;
        let id = self.gen_next_id;
        let weak = self.weak_self.clone();
        let playback = Playback::new(signal, &*self.output, Arc::new(move |event| {
            let arc = match weak.upgrade() { Some(arc) => arc, None => return };
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
                    },
                    playback::Event::Output(output::Event::Error(err)) => {
                        error!("{}", err);
                    },
                    playback::Event::State(state) => {
                        // GC tracks that have been stopped.
                        if state == State::Stopped {
                            player.playing.remove(&id);
                        }
                    },
                    _ => (),
                }
            });
        }));
        self.playing.insert(id, (audio.clone(), playback));
        Ok((id, &mut self.playing.get_mut(&id).unwrap().1))
    }

    /// Stops all currently playing tracks, advances the queue cursor and starts playing the track
    /// at that position.
    ///
    /// If the cursor has reached the end of the queue, a track from the queue autofill, if any, is
    /// appended and played.
    pub fn play_next_from_queue(&mut self) -> Result<Option<(u64, &mut Playback)>, Error> {
        let index = self.queue_cursor
            .map(|i| i + 1)
            .unwrap_or(0);
        if index >= self.queue.len() {
            self.queue.extend(self.queue_autofill.next().map(Arc::new).into_iter());
        }
        let audio = match self.queue.get(index) {
            Some(audio) => audio.clone(),
            None => return Ok(None),
        };
        self.playing.clear();
        self.queue_cursor = Some(index);
        let (id, mut pb) = self.init_playback(audio)?;
        pb.set_state(State::Playing);
        Ok(Some((id, pb)))
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
    fn from(err: Box<error::Error>) -> Error { Error::Other(err) }
}
