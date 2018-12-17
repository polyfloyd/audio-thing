use crate::audio::*;
use crate::filter::*;
use crate::player::output;
use sample;
use std::sync::{Arc, Condvar, Mutex};
use std::*;

#[derive(Debug)]
pub enum Event {
    Position(u64),
    State(State),
    Tempo(f64),
    Output(output::Event),
}

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum State {
    Playing,
    Paused,
    Stopped,
}

pub struct Playback {
    pub stream: Box<output::Stream>,

    sample_rate: u32,
    flow_state: Arc<(Condvar, Mutex<State>)>,
    sample_counter: Arc<Mutex<u64>>,

    tempo: Option<Arc<Mutex<f64>>>,
    seekable: Option<Arc<Mutex<Seekable + Send>>>,

    event_handler: Arc<Fn(Event) + Send + Sync>,
}

impl Playback {
    /// Initializes a new Playback. Playback should be started manually by setting the playstate to
    /// Playing.
    pub fn new(
        audio: dynam::Audio,
        output: &output::Output,
        event_handler: Arc<Fn(Event) + Send + Sync>,
    ) -> Playback {
        match audio {
            dynam::Audio::Source(source) => Playback::from_source(source, output, event_handler),
            dynam::Audio::Seek(seek) => Playback::from_seek(seek, output, event_handler),
        }
    }

    fn from_source(
        source: dynam::Source,
        output: &output::Output,
        event_handler: Arc<Fn(Event) + Send + Sync>,
    ) -> Playback {
        let flow_state = Arc::new((Condvar::new(), Mutex::new(State::Paused)));
        let sample_counter = Arc::new(Mutex::new(0));

        fn with_control<I>(
            source: I,
            fs: &Arc<(Condvar, Mutex<State>)>,
            sc: &Arc<Mutex<u64>>,
        ) -> Box<Source<Item = I::Item> + Send>
        where
            I: Source + Send + 'static,
            I::Item: sample::Frame,
        {
            Box::from(source.flow_control(fs.clone()).count_samples(sc.clone()))
        }
        let source_out = match source {
            dynam::Source::MonoI8(s) => {
                dynam::Source::MonoI8(with_control(s, &flow_state, &sample_counter))
            }
            dynam::Source::MonoU8(s) => {
                dynam::Source::MonoU8(with_control(s, &flow_state, &sample_counter))
            }
            dynam::Source::MonoI16(s) => {
                dynam::Source::MonoI16(with_control(s, &flow_state, &sample_counter))
            }
            dynam::Source::MonoU16(s) => {
                dynam::Source::MonoU16(with_control(s, &flow_state, &sample_counter))
            }
            dynam::Source::MonoI24(s) => {
                dynam::Source::MonoI24(with_control(s, &flow_state, &sample_counter))
            }
            dynam::Source::MonoU24(s) => {
                dynam::Source::MonoU24(with_control(s, &flow_state, &sample_counter))
            }
            dynam::Source::MonoI32(s) => {
                dynam::Source::MonoI32(with_control(s, &flow_state, &sample_counter))
            }
            dynam::Source::MonoU32(s) => {
                dynam::Source::MonoU32(with_control(s, &flow_state, &sample_counter))
            }
            dynam::Source::MonoI64(s) => {
                dynam::Source::MonoI64(with_control(s, &flow_state, &sample_counter))
            }
            dynam::Source::MonoU64(s) => {
                dynam::Source::MonoU64(with_control(s, &flow_state, &sample_counter))
            }
            dynam::Source::MonoF32(s) => {
                dynam::Source::MonoF32(with_control(s, &flow_state, &sample_counter))
            }
            dynam::Source::MonoF64(s) => {
                dynam::Source::MonoF64(with_control(s, &flow_state, &sample_counter))
            }
            dynam::Source::StereoI8(s) => {
                dynam::Source::StereoI8(with_control(s, &flow_state, &sample_counter))
            }
            dynam::Source::StereoU8(s) => {
                dynam::Source::StereoU8(with_control(s, &flow_state, &sample_counter))
            }
            dynam::Source::StereoI16(s) => {
                dynam::Source::StereoI16(with_control(s, &flow_state, &sample_counter))
            }
            dynam::Source::StereoU16(s) => {
                dynam::Source::StereoU16(with_control(s, &flow_state, &sample_counter))
            }
            dynam::Source::StereoI24(s) => {
                dynam::Source::StereoI24(with_control(s, &flow_state, &sample_counter))
            }
            dynam::Source::StereoU24(s) => {
                dynam::Source::StereoU24(with_control(s, &flow_state, &sample_counter))
            }
            dynam::Source::StereoI32(s) => {
                dynam::Source::StereoI32(with_control(s, &flow_state, &sample_counter))
            }
            dynam::Source::StereoU32(s) => {
                dynam::Source::StereoU32(with_control(s, &flow_state, &sample_counter))
            }
            dynam::Source::StereoI64(s) => {
                dynam::Source::StereoI64(with_control(s, &flow_state, &sample_counter))
            }
            dynam::Source::StereoU64(s) => {
                dynam::Source::StereoU64(with_control(s, &flow_state, &sample_counter))
            }
            dynam::Source::StereoF32(s) => {
                dynam::Source::StereoF32(with_control(s, &flow_state, &sample_counter))
            }
            dynam::Source::StereoF64(s) => {
                dynam::Source::StereoF64(with_control(s, &flow_state, &sample_counter))
            }
        };

        let eh_sub = event_handler.clone();
        let sub_handler = Arc::new(move |event| {
            if let output::Event::End = event {
                eh_sub(Event::State(State::Stopped));
            }
            eh_sub(Event::Output(event));
        });
        Playback {
            sample_rate: source_out.sample_rate(),
            stream: output.consume(source_out, sub_handler).unwrap(),
            flow_state,
            sample_counter,
            tempo: None,
            seekable: None,
            event_handler,
        }
    }

    fn from_seek(
        seek: dynam::Seek,
        output: &output::Output,
        event_handler: Arc<Fn(Event) + Send + Sync>,
    ) -> Playback {
        let flow_state = Arc::new((Condvar::new(), Mutex::new(State::Paused)));
        let sample_counter = Arc::new(Mutex::new(0));
        let tempo = Arc::new(Mutex::new(1.0));

        fn with_control<I>(
            seek: I,
            fs: &Arc<(Condvar, Mutex<State>)>,
            sc: &Arc<Mutex<u64>>,
            t: &Arc<Mutex<f64>>,
        ) -> (
            Box<Source<Item = I::Item> + Send>,
            Arc<Mutex<Seekable + Send>>,
        )
        where
            I: Seek + Send + 'static,
            I::Item: sample::Frame + Send,
            <I::Item as sample::Frame>::Float: Send,
            <I::Item as sample::Frame>::Sample: sample::ToSample<f64>
                + sample::FromSample<f64>
                + sample::FromSample<
                    <<I::Item as sample::Frame>::Float as sample::Frame>::Sample,
                > + Send
                + 'static,
        {
            let shared_seek = seek.shared();
            let mut_seek = shared_seek.input.clone();
            let source_out = shared_seek
                .stft(1024)
                .adjust_tempo(t.clone())
                .inverse()
                .flow_control(fs.clone())
                .count_samples(sc.clone());
            (Box::from(source_out), mut_seek)
        }
        let (source_out, mut_seek) = match seek {
            dynam::Seek::MonoI8(s) => {
                let (o, m) = with_control(s, &flow_state, &sample_counter, &tempo);
                (dynam::Source::MonoI8(o), m)
            }
            dynam::Seek::MonoU8(s) => {
                let (o, m) = with_control(s, &flow_state, &sample_counter, &tempo);
                (dynam::Source::MonoU8(o), m)
            }
            dynam::Seek::MonoI16(s) => {
                let (o, m) = with_control(s, &flow_state, &sample_counter, &tempo);
                (dynam::Source::MonoI16(o), m)
            }
            dynam::Seek::MonoU16(s) => {
                let (o, m) = with_control(s, &flow_state, &sample_counter, &tempo);
                (dynam::Source::MonoU16(o), m)
            }
            dynam::Seek::MonoI24(s) => {
                let (o, m) = with_control(s, &flow_state, &sample_counter, &tempo);
                (dynam::Source::MonoI24(o), m)
            }
            dynam::Seek::MonoU24(s) => {
                let (o, m) = with_control(s, &flow_state, &sample_counter, &tempo);
                (dynam::Source::MonoU24(o), m)
            }
            dynam::Seek::MonoI32(s) => {
                let (o, m) = with_control(s, &flow_state, &sample_counter, &tempo);
                (dynam::Source::MonoI32(o), m)
            }
            dynam::Seek::MonoU32(s) => {
                let (o, m) = with_control(s, &flow_state, &sample_counter, &tempo);
                (dynam::Source::MonoU32(o), m)
            }
            dynam::Seek::MonoI64(s) => {
                let (o, m) = with_control(s, &flow_state, &sample_counter, &tempo);
                (dynam::Source::MonoI64(o), m)
            }
            dynam::Seek::MonoU64(s) => {
                let (o, m) = with_control(s, &flow_state, &sample_counter, &tempo);
                (dynam::Source::MonoU64(o), m)
            }
            dynam::Seek::MonoF32(s) => {
                let (o, m) = with_control(s, &flow_state, &sample_counter, &tempo);
                (dynam::Source::MonoF32(o), m)
            }
            dynam::Seek::MonoF64(s) => {
                let (o, m) = with_control(s, &flow_state, &sample_counter, &tempo);
                (dynam::Source::MonoF64(o), m)
            }
            dynam::Seek::StereoI8(s) => {
                let (o, m) = with_control(s, &flow_state, &sample_counter, &tempo);
                (dynam::Source::StereoI8(o), m)
            }
            dynam::Seek::StereoU8(s) => {
                let (o, m) = with_control(s, &flow_state, &sample_counter, &tempo);
                (dynam::Source::StereoU8(o), m)
            }
            dynam::Seek::StereoI16(s) => {
                let (o, m) = with_control(s, &flow_state, &sample_counter, &tempo);
                (dynam::Source::StereoI16(o), m)
            }
            dynam::Seek::StereoU16(s) => {
                let (o, m) = with_control(s, &flow_state, &sample_counter, &tempo);
                (dynam::Source::StereoU16(o), m)
            }
            dynam::Seek::StereoI24(s) => {
                let (o, m) = with_control(s, &flow_state, &sample_counter, &tempo);
                (dynam::Source::StereoI24(o), m)
            }
            dynam::Seek::StereoU24(s) => {
                let (o, m) = with_control(s, &flow_state, &sample_counter, &tempo);
                (dynam::Source::StereoU24(o), m)
            }
            dynam::Seek::StereoI32(s) => {
                let (o, m) = with_control(s, &flow_state, &sample_counter, &tempo);
                (dynam::Source::StereoI32(o), m)
            }
            dynam::Seek::StereoU32(s) => {
                let (o, m) = with_control(s, &flow_state, &sample_counter, &tempo);
                (dynam::Source::StereoU32(o), m)
            }
            dynam::Seek::StereoI64(s) => {
                let (o, m) = with_control(s, &flow_state, &sample_counter, &tempo);
                (dynam::Source::StereoI64(o), m)
            }
            dynam::Seek::StereoU64(s) => {
                let (o, m) = with_control(s, &flow_state, &sample_counter, &tempo);
                (dynam::Source::StereoU64(o), m)
            }
            dynam::Seek::StereoF32(s) => {
                let (o, m) = with_control(s, &flow_state, &sample_counter, &tempo);
                (dynam::Source::StereoF32(o), m)
            }
            dynam::Seek::StereoF64(s) => {
                let (o, m) = with_control(s, &flow_state, &sample_counter, &tempo);
                (dynam::Source::StereoF64(o), m)
            }
        };

        let eh_sub = event_handler.clone();
        let sub_handler = Arc::new(move |event| {
            if let output::Event::End = event {
                eh_sub(Event::State(State::Stopped));
            }
            eh_sub(Event::Output(event));
        });
        Playback {
            sample_rate: source_out.sample_rate(),
            stream: output.consume(source_out, sub_handler).unwrap(),
            flow_state,
            sample_counter,
            tempo: Some(tempo),
            seekable: Some(mut_seek),
            event_handler,
        }
    }

    /// Returns the total number of samples in of the playing audio if known.
    pub fn duration(&self) -> Option<u64> {
        self.seekable
            .as_ref()
            .map(|s| (*s.lock().unwrap()).length())
    }

    /// Returns the duration as a Duration if known.
    pub fn duration_time(&self) -> Option<time::Duration> {
        self.duration()
            .map(|num_samples| duration_of(self.sample_rate, num_samples))
    }

    /// Returns the position of the sample that will be read next.
    /// If The audio is infinite, this will simply be the total number of samples played.
    pub fn position(&self) -> u64 {
        self.seekable
            .as_ref()
            .map(|s| s.lock().unwrap().current_position())
            .unwrap_or_else(|| *self.sample_counter.lock().unwrap())
    }

    /// Seeks to the sample at the specified position. If seeking is not supported, this is a
    /// no-op.
    pub fn set_position(&mut self, position: u64) {
        self.seekable
            .as_ref()
            .map(|s| s.lock().unwrap().seek(position))
            .unwrap_or(Ok(()))
            .unwrap(); // FIXME
        (self.event_handler)(Event::Position(position));
    }

    /// Seeks using a duration.
    pub fn set_position_time(&mut self, timestamp: time::Duration) {
        let secs = timestamp.as_secs() * u64::from(self.sample_rate);
        self.set_position(secs);
    }

    /// Returns the current position as a Duration.
    pub fn position_time(&self) -> time::Duration {
        duration_of(self.sample_rate, self.position())
    }

    pub fn state(&self) -> State {
        *self.flow_state.1.lock().unwrap()
    }

    pub fn set_state(&mut self, state: State) {
        let &(ref cvar, ref lock) = &*self.flow_state;
        let mut cur_state = lock.lock().unwrap();
        if *cur_state != State::Stopped {
            *cur_state = state;
        }
        cvar.notify_all();
        (self.event_handler)(Event::State(state));
    }

    pub fn tempo(&self) -> f64 {
        self.tempo
            .as_ref()
            .map(|t| *t.lock().unwrap())
            .unwrap_or(1.0)
    }

    /// Sets the tempo for the currently playing audio.
    /// This is a no-op if the tempo of the audio can not be altered or the tempo specified is
    /// invalid: `tempo <= 0.0`.
    pub fn set_tempo(&mut self, tempo: f64) {
        if tempo > 0.0 {
            if let Some(ref t) = self.tempo {
                *t.lock().unwrap() = tempo;
                (self.event_handler)(Event::Tempo(tempo));
            }
        }
    }
}

/// FlowControl acts as a part of a signal pipeline allowing the flow to be paused and stopped.
/// Because pausing works by blocking any calls to next, `FlowControl` provides its own concurrency
/// method instead of recommending `audio::Shared`.
struct FlowControl<S>
where
    S: Source,
    S::Item: sample::Frame,
{
    pub state: Arc<(Condvar, Mutex<State>)>,
    input: S,
}

impl<S> iter::Iterator for FlowControl<S>
where
    S: Source,
    S::Item: sample::Frame,
{
    type Item = S::Item;
    fn next(&mut self) -> Option<Self::Item> {
        let &(ref cvar, ref lock) = &*self.state;

        let mut state = lock.lock().unwrap();
        while *state == State::Paused {
            state = cvar.wait(state).unwrap();
        }

        match *state {
            State::Paused => unreachable!(),
            State::Stopped => None,
            State::Playing => {
                let f = self.input.next();
                if f.is_none() {
                    *state = State::Stopped;
                }
                f
            }
        }
    }
}

impl<S> Source for FlowControl<S>
where
    S: Source,
    S::Item: sample::Frame,
{
    fn sample_rate(&self) -> u32 {
        self.input.sample_rate()
    }
}

impl<S> Drop for FlowControl<S>
where
    S: Source,
    S::Item: sample::Frame,
{
    // If the state is set to paused, another thread attempting to read from the stream is blocked.
    // Here, we set the state to stopped when this FlowControl is dropped, so that the reading
    // thread will never deadlock.
    fn drop(&mut self) {
        let &(ref cvar, ref lock) = &*self.state;
        *lock.lock().unwrap() = State::Stopped;
        cvar.notify_all();
    }
}

trait IntoFlowControl: Source + Sized
where
    Self::Item: sample::Frame,
{
    fn flow_control(self, state: Arc<(Condvar, Mutex<State>)>) -> FlowControl<Self> {
        FlowControl { state, input: self }
    }
}

impl<T> IntoFlowControl for T
where
    T: Source,
    T::Item: sample::Frame,
{
}

struct SampleCounter<S>
where
    S: Source,
    S::Item: sample::Frame,
{
    pub counter: Arc<Mutex<u64>>,
    input: S,
}

impl<S> iter::Iterator for SampleCounter<S>
where
    S: Source,
    S::Item: sample::Frame,
{
    type Item = S::Item;
    fn next(&mut self) -> Option<Self::Item> {
        let n = self.input.next();
        if n.is_some() {
            *self.counter.lock().unwrap() += 1;
        }
        n
    }
}

impl<S> Source for SampleCounter<S>
where
    S: Source,
    S::Item: sample::Frame,
{
    fn sample_rate(&self) -> u32 {
        self.input.sample_rate()
    }
}

trait IntoSampleCounter: Source + Sized
where
    Self::Item: sample::Frame,
{
    fn count_samples(self, counter: Arc<Mutex<u64>>) -> SampleCounter<Self> {
        SampleCounter {
            counter,
            input: self,
        }
    }
}

impl<T> IntoSampleCounter for T
where
    T: Source,
    T::Item: sample::Frame,
{
}
