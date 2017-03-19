use std::*;
use std::sync::{Arc, Condvar, Mutex};
use sample;
use ::audio::*;
use ::filter::*;
use ::player::output;


#[derive(PartialEq, Eq, Copy, Clone)]
pub enum State {
    Playing,
    Paused,
    Stopped,
}


pub struct Playback {
    stream: Box<output::Stream>,
    flow_state: Arc<(Condvar, Mutex<State>)>,
    sample_counter: Arc<Mutex<u64>>,

    tempo: Option<Arc<Mutex<f64>>>,
    seek: Option<Arc<Mutex<Seekable>>>,
}

pub fn play(audio: dyn::Audio, output: Arc<output::Output>) -> Playback {
    match audio {
        dyn::Audio::Source(source) => {
            Playback::from_source(source, output)
        },
        dyn::Audio::Seek(seek) => {
            Playback::from_seek(seek, output)
        },
    }
}

impl Playback {
    fn from_source(source: dyn::Source, output: Arc<output::Output>) -> Playback {
        let flow_state = Arc::new((Condvar::new(), Mutex::new(State::Playing)));
        let sample_counter = Arc::new(Mutex::new(0));

        fn with_control<I>(source: I, fs: &Arc<(Condvar, Mutex<State>)>, sc: &Arc<Mutex<u64>>) -> Box<Source<Item=I::Item> + Send>
            where I: Source + Send + 'static,
                  I::Item: sample::Frame {
            Box::from(source
                .flow_control(fs.clone())
                .count_samples(sc.clone()))
        }
        let source_out = match source {
            dyn::Source::MonoI8(s) => dyn::Source::MonoI8(with_control(s, &flow_state, &sample_counter)),
            dyn::Source::MonoU8(s) => dyn::Source::MonoU8(with_control(s, &flow_state, &sample_counter)),
            dyn::Source::MonoI16(s) => dyn::Source::MonoI16(with_control(s, &flow_state, &sample_counter)),
            dyn::Source::MonoU16(s) => dyn::Source::MonoU16(with_control(s, &flow_state, &sample_counter)),
            dyn::Source::MonoI32(s) => dyn::Source::MonoI32(with_control(s, &flow_state, &sample_counter)),
            dyn::Source::MonoU32(s) => dyn::Source::MonoU32(with_control(s, &flow_state, &sample_counter)),
            dyn::Source::MonoI64(s) => dyn::Source::MonoI64(with_control(s, &flow_state, &sample_counter)),
            dyn::Source::MonoU64(s) => dyn::Source::MonoU64(with_control(s, &flow_state, &sample_counter)),
            dyn::Source::MonoF32(s) => dyn::Source::MonoF32(with_control(s, &flow_state, &sample_counter)),
            dyn::Source::MonoF64(s) => dyn::Source::MonoF64(with_control(s, &flow_state, &sample_counter)),
            dyn::Source::StereoI8(s) => dyn::Source::StereoI8(with_control(s, &flow_state, &sample_counter)),
            dyn::Source::StereoU8(s) => dyn::Source::StereoU8(with_control(s, &flow_state, &sample_counter)),
            dyn::Source::StereoI16(s) => dyn::Source::StereoI16(with_control(s, &flow_state, &sample_counter)),
            dyn::Source::StereoU16(s) => dyn::Source::StereoU16(with_control(s, &flow_state, &sample_counter)),
            dyn::Source::StereoI32(s) => dyn::Source::StereoI32(with_control(s, &flow_state, &sample_counter)),
            dyn::Source::StereoU32(s) => dyn::Source::StereoU32(with_control(s, &flow_state, &sample_counter)),
            dyn::Source::StereoI64(s) => dyn::Source::StereoI64(with_control(s, &flow_state, &sample_counter)),
            dyn::Source::StereoU64(s) => dyn::Source::StereoU64(with_control(s, &flow_state, &sample_counter)),
            dyn::Source::StereoF32(s) => dyn::Source::StereoF32(with_control(s, &flow_state, &sample_counter)),
            dyn::Source::StereoF64(s) => dyn::Source::StereoF64(with_control(s, &flow_state, &sample_counter)),
        };

        Playback {
            stream: output.consume(source_out).unwrap(),
            flow_state: flow_state,
            sample_counter: sample_counter,
            tempo: None,
            seek: None,
        }
    }

    fn from_seek(seek: dyn::Seek, output: Arc<output::Output>) -> Playback {
        let flow_state = Arc::new((Condvar::new(), Mutex::new(State::Playing)));
        let sample_counter = Arc::new(Mutex::new(0));
        let tempo = Arc::new(Mutex::new(1.0));

        fn with_control<I>(seek: I, fs: &Arc<(Condvar, Mutex<State>)>, sc: &Arc<Mutex<u64>>, t: &Arc<Mutex<f64>>) -> (Box<Source<Item=I::Item> + Send>, Arc<Mutex<Seekable>>)
            where I: Seek + Send + 'static,
                  I::Item: sample::Frame + Send,
                  <I::Item as sample::Frame>::Float: Send,
                  <I::Item as sample::Frame>::Sample: sample::ToSample<f64> +
                                                      sample::FromSample<f64> +
                                                      sample::FromSample<<<I::Item as sample::Frame>::Float as sample::Frame>::Sample> +
                                                      Send + 'static {
                let shared_seek = seek
                    .shared();
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
            dyn::Seek::MonoI8(s) => {
                let (o, m) = with_control(s, &flow_state, &sample_counter, &tempo);
                (dyn::Source::MonoI8(o), m)
            },
            dyn::Seek::MonoU8(s) => {
                let (o, m) = with_control(s, &flow_state, &sample_counter, &tempo);
                (dyn::Source::MonoU8(o), m)
            },
            dyn::Seek::MonoI16(s) => {
                let (o, m) = with_control(s, &flow_state, &sample_counter, &tempo);
                (dyn::Source::MonoI16(o), m)
            },
            dyn::Seek::MonoU16(s) => {
                let (o, m) = with_control(s, &flow_state, &sample_counter, &tempo);
                (dyn::Source::MonoU16(o), m)
            },
            dyn::Seek::MonoI32(s) => {
                let (o, m) = with_control(s, &flow_state, &sample_counter, &tempo);
                (dyn::Source::MonoI32(o), m)
            },
            dyn::Seek::MonoU32(s) => {
                let (o, m) = with_control(s, &flow_state, &sample_counter, &tempo);
                (dyn::Source::MonoU32(o), m)
            },
            dyn::Seek::MonoI64(s) => {
                let (o, m) = with_control(s, &flow_state, &sample_counter, &tempo);
                (dyn::Source::MonoI64(o), m)
            },
            dyn::Seek::MonoU64(s) => {
                let (o, m) = with_control(s, &flow_state, &sample_counter, &tempo);
                (dyn::Source::MonoU64(o), m)
            },
            dyn::Seek::MonoF32(s) => {
                let (o, m) = with_control(s, &flow_state, &sample_counter, &tempo);
                (dyn::Source::MonoF32(o), m)
            },
            dyn::Seek::MonoF64(s) => {
                let (o, m) = with_control(s, &flow_state, &sample_counter, &tempo);
                (dyn::Source::MonoF64(o), m)
            },
            dyn::Seek::StereoI8(s) => {
                let (o, m) = with_control(s, &flow_state, &sample_counter, &tempo);
                (dyn::Source::StereoI8(o), m)
            },
            dyn::Seek::StereoU8(s) => {
                let (o, m) = with_control(s, &flow_state, &sample_counter, &tempo);
                (dyn::Source::StereoU8(o), m)
            },
            dyn::Seek::StereoI16(s) => {
                let (o, m) = with_control(s, &flow_state, &sample_counter, &tempo);
                (dyn::Source::StereoI16(o), m)
            },
            dyn::Seek::StereoU16(s) => {
                let (o, m) = with_control(s, &flow_state, &sample_counter, &tempo);
                (dyn::Source::StereoU16(o), m)
            },
            dyn::Seek::StereoI32(s) => {
                let (o, m) = with_control(s, &flow_state, &sample_counter, &tempo);
                (dyn::Source::StereoI32(o), m)
            },
            dyn::Seek::StereoU32(s) => {
                let (o, m) = with_control(s, &flow_state, &sample_counter, &tempo);
                (dyn::Source::StereoU32(o), m)
            },
            dyn::Seek::StereoI64(s) => {
                let (o, m) = with_control(s, &flow_state, &sample_counter, &tempo);
                (dyn::Source::StereoI64(o), m)
            },
            dyn::Seek::StereoU64(s) => {
                let (o, m) = with_control(s, &flow_state, &sample_counter, &tempo);
                (dyn::Source::StereoU64(o), m)
            },
            dyn::Seek::StereoF32(s) => {
                let (o, m) = with_control(s, &flow_state, &sample_counter, &tempo);
                (dyn::Source::StereoF32(o), m)
            },
            dyn::Seek::StereoF64(s) => {
                let (o, m) = with_control(s, &flow_state, &sample_counter, &tempo);
                (dyn::Source::StereoF64(o), m)
            },
        };

        Playback {
            stream: output.consume(source_out).unwrap(),
            flow_state: flow_state,
            sample_counter: sample_counter,
            tempo: Some(tempo),
            seek: Some(mut_seek),
        }
    }

    /// Returns the total number of samples in of the playing audio if known.
    pub fn duration(&self) -> Option<u64> {
        self.seek
            .as_ref()
            .map(|s| (*s.lock().unwrap()).length())
    }

    /// Returns the position of the sample that will be read next.
    /// If The audio is infinite, this will simply be the total number of samples played.
    pub fn position(&self) -> u64 {
        self.seek
            .as_ref()
            .map(|s| s.lock().unwrap().position() )
            .unwrap_or(*self.sample_counter.lock().unwrap())
    }

    pub fn playstate(&self) -> State {
        *self.flow_state.1.lock().unwrap()
    }

    pub fn set_playstate(&mut self, state: State) {
        let &(ref cvar, ref lock) = &*self.flow_state;
        let mut cur_state = lock.lock().unwrap();
        if *cur_state != State::Stopped {
            *cur_state = state;
        }
        cvar.notify_all();
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
            }
        }
    }
}


/// FlowControl acts as a part of a signal pipeline allowing the flow to be paused and stopped.
/// Because pausing works by blocking any calls to next, `FlowControl` provides its own concurrency
/// method instead of recommending `audio::Shared`.
struct FlowControl<S>
    where S: Source,
          S::Item: sample::Frame {
    pub state: Arc<(Condvar, Mutex<State>)>,
    input: S,
}

impl<S> iter::Iterator for FlowControl<S>
    where S: Source,
          S::Item: sample::Frame {
    type Item = S::Item;
    fn next(&mut self) -> Option<Self::Item> {
        let &(ref cvar, ref lock) = &*self.state;

        let mut state = lock.lock().unwrap();
        while *state == State::Paused {
            state = cvar.wait(state).unwrap();
        }

        match *state {
            State::Paused  => unreachable!(),
            State::Stopped => return None,
            State::Playing => {
                let f = self.input.next();
                if f.is_none() {
                    *state = State::Stopped;
                }
                return f
            },
        }
    }
}

impl<S> Source for FlowControl<S>
    where S: Source,
          S::Item: sample::Frame {
    fn sample_rate(&self) -> u32 { self.input.sample_rate() }
}

trait IntoFlowControl: Source + Sized
    where Self::Item: sample::Frame {
    fn flow_control(self, state: Arc<(Condvar, Mutex<State>)>) -> FlowControl<Self> {
        FlowControl{ state: state, input: self }
    }
}

impl<T> IntoFlowControl for T
    where T: Source,
          T::Item: sample::Frame { }


struct SampleCounter<S>
    where S: Source,
          S::Item: sample::Frame {
    pub counter: Arc<Mutex<u64>>,
    input: S,
}

impl<S> iter::Iterator for SampleCounter<S>
    where S: Source,
          S::Item: sample::Frame {
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
    where S: Source,
          S::Item: sample::Frame {
    fn sample_rate(&self) -> u32 { self.input.sample_rate() }
}

trait IntoSampleCounter: Source + Sized
    where Self::Item: sample::Frame {
    fn count_samples(self, counter: Arc<Mutex<u64>>) -> SampleCounter<Self> {
        SampleCounter{ counter: counter, input: self }
    }
}

impl<T> IntoSampleCounter for T
    where T: Source,
          T::Item: sample::Frame { }
