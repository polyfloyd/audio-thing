use audio::*;
use player::output;
use pulse;
use sample::{self, Frame, I24, Sample};
use std::sync::{Arc, Mutex};
use std::*;

pub struct Output();

impl super::Output for Output {
    fn consume(
        &self,
        source: dyn::Source,
        eh: Arc<Fn(output::Event) + Send + Sync>,
    ) -> Result<Box<super::Stream>, Box<error::Error>> {
        let sr = source.sample_rate();
        // Pulseaudio does not support too many formats, so we coerce the sample format when
        // needed.
        match source {
            dyn::Source::MonoI8(source) => Stream::new(
                source
                    .map(|f| -> [u8; 1] { f.map(Sample::from_sample) })
                    .source(sr),
                eh,
            ),
            dyn::Source::MonoU8(source) => Stream::new(source, eh),
            dyn::Source::MonoI16(source) => Stream::new(source, eh),
            dyn::Source::MonoU16(source) => Stream::new(
                source
                    .map(|f| -> [i16; 1] { f.map(Sample::from_sample) })
                    .source(sr),
                eh,
            ),
            dyn::Source::MonoI24(source) => Stream::new(source, eh),
            dyn::Source::MonoU24(source) => Stream::new(
                source
                    .map(|f| -> [I24; 1] { f.map(Sample::from_sample) })
                    .source(sr),
                eh,
            ),
            dyn::Source::MonoI32(source) => Stream::new(
                source
                    .map(|f| -> [f32; 1] { f.map(Sample::from_sample) })
                    .source(sr),
                eh,
            ),
            dyn::Source::MonoU32(source) => Stream::new(
                source
                    .map(|f| -> [f32; 1] { f.map(Sample::from_sample) })
                    .source(sr),
                eh,
            ),
            dyn::Source::MonoI64(source) => Stream::new(
                source
                    .map(|f| -> [f32; 1] { f.map(Sample::from_sample) })
                    .source(sr),
                eh,
            ),
            dyn::Source::MonoU64(source) => Stream::new(
                source
                    .map(|f| -> [f32; 1] { f.map(Sample::from_sample) })
                    .source(sr),
                eh,
            ),
            dyn::Source::MonoF32(source) => Stream::new(source, eh),
            dyn::Source::MonoF64(source) => Stream::new(
                source
                    .map(|f| -> [f32; 1] { f.map(Sample::from_sample) })
                    .source(sr),
                eh,
            ),
            dyn::Source::StereoI8(source) => Stream::new(
                source
                    .map(|f| -> [u8; 2] { f.map(Sample::from_sample) })
                    .source(sr),
                eh,
            ),
            dyn::Source::StereoU8(source) => Stream::new(source, eh),
            dyn::Source::StereoI16(source) => Stream::new(source, eh),
            dyn::Source::StereoU16(source) => Stream::new(
                source
                    .map(|f| -> [i16; 2] { f.map(Sample::from_sample) })
                    .source(sr),
                eh,
            ),
            dyn::Source::StereoI24(source) => Stream::new(source, eh),
            dyn::Source::StereoU24(source) => Stream::new(
                source
                    .map(|f| -> [I24; 2] { f.map(Sample::from_sample) })
                    .source(sr),
                eh,
            ),
            dyn::Source::StereoI32(source) => Stream::new(
                source
                    .map(|f| -> [f32; 2] { f.map(Sample::from_sample) })
                    .source(sr),
                eh,
            ),
            dyn::Source::StereoU32(source) => Stream::new(
                source
                    .map(|f| -> [f32; 2] { f.map(Sample::from_sample) })
                    .source(sr),
                eh,
            ),
            dyn::Source::StereoI64(source) => Stream::new(
                source
                    .map(|f| -> [f32; 2] { f.map(Sample::from_sample) })
                    .source(sr),
                eh,
            ),
            dyn::Source::StereoU64(source) => Stream::new(
                source
                    .map(|f| -> [f32; 2] { f.map(Sample::from_sample) })
                    .source(sr),
                eh,
            ),
            dyn::Source::StereoF32(source) => Stream::new(source, eh),
            dyn::Source::StereoF64(source) => Stream::new(
                source
                    .map(|f| -> [f32; 2] { f.map(Sample::from_sample) })
                    .source(sr),
                eh,
            ),
        }
    }
}

struct Stream<S>
where
    S: Source,
    S::Item: sample::Frame,
{
    /// The sink to which the stream is being written to along with a bit to indicate whether the
    /// stream should be closed.
    sink: Arc<Mutex<(pulse::Sink<S::Item>, bool)>>,

    event_handler: Arc<Fn(output::Event) + Send + Sync>,
}

impl<S> Stream<S>
where
    S: Source,
    S::Item: sample::Frame,
{
    fn new(
        source: S,
        event_handler: Arc<Fn(output::Event) + Send + Sync>,
    ) -> Result<Box<super::Stream>, Box<error::Error>>
    where
        S: Source + Send + 'static,
        S::Item: sample::Frame + Send,
        <S::Item as sample::Frame>::Sample: Sample + pulse::AsSampleFormat,
    {
        let app_name = format!("{} v{}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
        let pulse_sink = pulse::sink(&app_name, "TODO", source.sample_rate())?;
        let sink = Arc::new(Mutex::new((pulse_sink, false)));

        let eh_sub = event_handler.clone();
        let sub_handler = Arc::new(Mutex::new(move |event| eh_sub(event)));
        let sink_out = sink.clone();
        thread::spawn(move || {
            for frame in source {
                let mut out_state = sink_out.lock().unwrap();
                if out_state.1 {
                    return;
                }
                if let Err(err) = out_state.0.write_frame(frame) {
                    sub_handler.lock().unwrap()(output::Event::Error(err));
                    return;
                }
            }
            sub_handler.lock().unwrap()(output::Event::End);
        });
        Ok(Box::new(Stream::<S> {
            sink: sink,
            event_handler: event_handler,
        }))
    }
}

impl<S> super::Stream for Stream<S>
where
    S: Source,
    S::Item: sample::Frame,
{
    fn volume(&self) -> Result<f64, Box<error::Error>> {
        unimplemented!();
    }

    fn set_volume(&mut self, volume: f64) -> Result<(), Box<error::Error>> {
        (self.event_handler)(output::Event::Volume(volume));
        unimplemented!();
    }

    fn latency(&self) -> Result<time::Duration, Box<error::Error>> {
        let out_state = self.sink.lock().unwrap();
        out_state.0.connection().latency().map_err(Box::from)
    }
}

impl<S> Drop for Stream<S>
where
    S: Source,
    S::Item: sample::Frame,
{
    fn drop(&mut self) {
        let mut out_state = self.sink.lock().unwrap();
        (*out_state).1 = true;
    }
}
