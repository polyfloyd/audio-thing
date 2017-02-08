use std::*;
use std::sync::mpsc;
use sample;

mod swresample {
    #![allow(dead_code, non_snake_case, non_camel_case_types, non_upper_case_globals, improper_ctypes)]
    include!(concat!(env!("OUT_DIR"), "/swresample.rs"));
}
use self::swresample::*;

pub struct Resampler<O, I, S>
    where O: sample::Frame,
          I: sample::Frame,
          S: sample::Signal<Item=I> {
    swr: *mut SwrContext,
    output: collections::VecDeque<O::Sample>,
    input: S,
    output_rate: i64,
    input_rate: i64,
}

impl<O, I, S> Resampler<O, I, S>
    where O: sample::Frame,
          I: sample::Frame,
          S: sample::Signal<Item=I>,
          O::Sample: AsSampleFormat,
          I::Sample: AsSampleFormat,
          O::NumChannels: AsChannelLayout,
          I::NumChannels: AsChannelLayout {
    pub fn new(output_rate: u32, input_rate: u32, input: S) -> Resampler<O, I, S> {
        let swr = unsafe {
            let swr = swr_alloc_set_opts(
                ptr::null_mut(),                         // we're allocating a new context
                O::NumChannels::channel_layout() as i64, // out_ch_layout
                O::Sample::sample_format(),              // out_sample_fmt
                output_rate as i32,                      // out_sample_rate
                I::NumChannels::channel_layout() as i64, // in_ch_layout
                I::Sample::sample_format(),              // in_sample_fmt
                input_rate as i32,                       // in_sample_rate
                0,                                       // log_offset
                ptr::null_mut(),                         // log_ctx
            );
            swr_init(swr);
            swr
        };
        Resampler {
            swr: swr,
            output: collections::VecDeque::new(),
            input: input,
            output_rate: output_rate as i64,
            input_rate: input_rate as i64,
        }
    }
}

impl<O, I, S> iter::Iterator for Resampler<O, I, S>
    where O: sample::Frame,
          I: sample::Frame,
          S: sample::Signal<Item=I>,
          O::Sample: AsSampleFormat {
    type Item = O;
    fn next(&mut self) -> Option<Self::Item> {
        while self.output.len() < O::n_channels() {
            let frame_in = match self.input.next() {
                Some(frame) => frame,
                None        => break,
            };
            unsafe {
                let in_samples = O::n_channels();
                let mut output: *mut u8 = ptr::null_mut();
                let out_samples = av_rescale_rnd(
                    swr_get_delay(self.swr, self.input_rate) + in_samples as i64,
                    self.output_rate,
                    self.input_rate,
                    AVRounding::AV_ROUND_UP,
                );
                av_samples_alloc(
                    &mut output,                // data pointer
                    ptr::null_mut(),            // linesize
                    O::n_channels() as i32,     // num output channels
                    out_samples as i32,         // num output samples
                    O::Sample::sample_format(), // output format
                    0,                          // align
                );
                let mut input = mem::transmute::<&I, *const u8>(&frame_in);
                let out_samples = swr_convert(
                    self.swr,
                    &mut output,
                    out_samples as i32,
                    &mut input,
                    in_samples as i32,
                );
                let sl = slice::from_raw_parts(output as *const O::Sample, out_samples as usize);
                for sample_out in sl {
                    self.output.push_back(*sample_out);
                }
                av_freep(mem::transmute::<*mut *mut u8, *mut os::raw::c_void>(&mut output));
            }
        }
        if self.output.len() >= O::n_channels() {
            O::from_samples(&mut self.output.drain(..O::n_channels()))
        } else {
            None
        }
    }
}

impl<O, I, S> Drop for Resampler<O, I, S>
    where O: sample::Frame,
          I: sample::Frame,
          S: sample::Signal<Item=I> {
    fn drop(&mut self) {
        unsafe {
            swr_free(&mut self.swr);
        }
    }
}


pub trait AsSampleFormat {
    fn sample_format() -> AVSampleFormat;
}

impl AsSampleFormat for u8 {
    fn sample_format() -> AVSampleFormat { AVSampleFormat::AV_SAMPLE_FMT_U8 }
}

impl AsSampleFormat for i16 {
    fn sample_format() -> AVSampleFormat { AVSampleFormat::AV_SAMPLE_FMT_S16 }
}

impl AsSampleFormat for i32 {
    fn sample_format() -> AVSampleFormat { AVSampleFormat::AV_SAMPLE_FMT_S32 }
}

impl AsSampleFormat for i64 {
    fn sample_format() -> AVSampleFormat { AVSampleFormat::AV_SAMPLE_FMT_S64 }
}

impl AsSampleFormat for f32 {
    fn sample_format() -> AVSampleFormat { AVSampleFormat::AV_SAMPLE_FMT_FLT }
}

impl AsSampleFormat for f64 {
    fn sample_format() -> AVSampleFormat { AVSampleFormat::AV_SAMPLE_FMT_DBL }
}

pub trait AsChannelLayout {
    fn channel_layout() -> u32;
}

impl AsChannelLayout for sample::frame::N1 {
    fn channel_layout() -> u32 { AV_CH_LAYOUT_MONO }
}

impl AsChannelLayout for sample::frame::N2 {
    fn channel_layout() -> u32 { AV_CH_LAYOUT_STEREO }
}
