use std::*;
use std::sync::{Arc, Mutex};
use sample::{self, Frame, Sample, I24};
use ::audio::*;
use ::pulse;

pub struct Output();

impl super::Output for Output {
    fn consume(&self, source: dyn::Source) -> Result<Box<super::Stream>, Box<error::Error>> {
        let sr = source.sample_rate();
        // Pulseaudio does not support too many formats, so we coerce the sample format when
        // needed.
        match source {
            dyn::Source::MonoI8(source) => Stream::new(source.map(|f| -> [u8; 1] { f.map(Sample::from_sample) }).source(sr)),
            dyn::Source::MonoU8(source) => Stream::new(source),
            dyn::Source::MonoI16(source) => Stream::new(source),
            dyn::Source::MonoU16(source) => Stream::new(source.map(|f| -> [i16; 1] { f.map(Sample::from_sample) }).source(sr)),
            dyn::Source::MonoI24(source) => Stream::new(source),
            dyn::Source::MonoU24(source) => Stream::new(source.map(|f| -> [I24; 1] { f.map(Sample::from_sample) }).source(sr)),
            dyn::Source::MonoI32(source) => Stream::new(source.map(|f| -> [f32; 1] { f.map(Sample::from_sample) }).source(sr)),
            dyn::Source::MonoU32(source) => Stream::new(source.map(|f| -> [f32; 1] { f.map(Sample::from_sample) }).source(sr)),
            dyn::Source::MonoI64(source) => Stream::new(source.map(|f| -> [f32; 1] { f.map(Sample::from_sample) }).source(sr)),
            dyn::Source::MonoU64(source) => Stream::new(source.map(|f| -> [f32; 1] { f.map(Sample::from_sample) }).source(sr)),
            dyn::Source::MonoF32(source) => Stream::new(source),
            dyn::Source::MonoF64(source) => Stream::new(source.map(|f| -> [f32; 1] { f.map(Sample::from_sample) }).source(sr)),
            dyn::Source::StereoI8(source) => Stream::new(source.map(|f| -> [u8; 2] { f.map(Sample::from_sample) }).source(sr)),
            dyn::Source::StereoU8(source) => Stream::new(source),
            dyn::Source::StereoI16(source) => Stream::new(source),
            dyn::Source::StereoU16(source) => Stream::new(source.map(|f| -> [i16; 2] { f.map(Sample::from_sample) }).source(sr)),
            dyn::Source::StereoI24(source) => Stream::new(source),
            dyn::Source::StereoU24(source) => Stream::new(source.map(|f| -> [I24; 2] { f.map(Sample::from_sample) }).source(sr)),
            dyn::Source::StereoI32(source) => Stream::new(source.map(|f| -> [f32; 2] { f.map(Sample::from_sample) }).source(sr)),
            dyn::Source::StereoU32(source) => Stream::new(source.map(|f| -> [f32; 2] { f.map(Sample::from_sample) }).source(sr)),
            dyn::Source::StereoI64(source) => Stream::new(source.map(|f| -> [f32; 2] { f.map(Sample::from_sample) }).source(sr)),
            dyn::Source::StereoU64(source) => Stream::new(source.map(|f| -> [f32; 2] { f.map(Sample::from_sample) }).source(sr)),
            dyn::Source::StereoF32(source) => Stream::new(source),
            dyn::Source::StereoF64(source) => Stream::new(source.map(|f| -> [f32; 2] { f.map(Sample::from_sample) }).source(sr)),
        }
    }
}

struct Stream<S>
    where S: Source,
          S::Item: sample::Frame {
    /// The sink to which the stream is being written to along with a bit to indicate whether the
    /// stream should be closed.
    sink: Arc<Mutex<(pulse::Sink<S::Item>, bool)>>,
}

impl<S> Stream<S>
    where S: Source,
          S::Item: sample::Frame {
    fn new(source: S) -> Result<Box<super::Stream>, Box<error::Error>>
        where S: Source + Send + 'static,
              S::Item: sample::Frame + Send,
              <S::Item as sample::Frame>::Sample: Sample + pulse::AsSampleFormat {
        let app_name = format!("{} v{}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
        let pulse_sink = pulse::sink(&app_name, "TODO", source.sample_rate())?;
        let sink = Arc::new(Mutex::new((pulse_sink, false)));

        let sink_out = sink.clone();
        thread::spawn(move || {
            for frame in source {
                let mut out_state = sink_out.lock().unwrap();
                if out_state.1 {
                    break;
                }
                if let Err(err) = out_state.0.write_frame(frame) {
                    error!("Error writing stream: {}", err);
                }
            }
        });
        Ok(Box::new(Stream::<S> {
            sink: sink,
        }))
    }
}

impl<S> super::Stream for Stream<S>
    where S: Source,
          S::Item: sample::Frame {
    fn volume(&self) -> Result<f64, Box<error::Error>> {
        unimplemented!();
    }

    fn set_volume(&mut self, volume: f64) -> Result<(), Box<error::Error>> {
        unimplemented!();
    }

    fn latency(&self) -> Result<time::Duration, Box<error::Error>> {
        let out_state = self.sink.lock().unwrap();
        out_state.0.connection()
            .latency()
            .map_err(Box::from)
    }
}

impl<S> Drop for Stream<S>
    where S: Source,
          S::Item: sample::Frame {
    fn drop(&mut self) {
        let mut out_state = self.sink.lock().unwrap();
        (*out_state).1 = true;
    }
}
