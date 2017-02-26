extern crate dft;
extern crate flac;
extern crate num_complex;
extern crate sample;
use std::*;
use ::audio::{Sink, Source};
use ::filter::Resample;

mod audio;
mod filter;
mod format;
mod pulse;

fn main() {
    let file = fs::File::open("test.flac").unwrap();
    let decoder = format::flac::Decoder::new(file).unwrap();

    let out_rate = decoder.sample_rate();
    let source = decoder.resample(out_rate);

    let mut sink: pulse::Sink<[i16; 2]> = pulse::sink("blarp", out_rate).unwrap();
    for frame in source {
        sink.write_frame(frame).unwrap();
    }
}
