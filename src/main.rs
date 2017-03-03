extern crate dft;
extern crate flac;
extern crate sample;
use std::*;
use std::io::BufRead;
use ::audio::{Sink, Source};
use ::filter::{AdjustTempo, IntoStft, Stft};

mod audio;
mod filter;
mod format;
mod pulse;

fn main() {
    let filename = env::args().nth(1)
        .expect("$1 should be an audio file");
    let tempo = env::args().nth(2)
        .and_then(|t| t.parse().ok())
        .expect("$2 should be the tempo");

    let file = fs::File::open(filename).unwrap();
    let input = format::flac::Decoder::new(file).unwrap();

    let tempo = input
        .stft(1024)
        .adjust_tempo(tempo);
    let ratio_mut = tempo.ratio.clone();
    let source = tempo
        .inverse();

    thread::spawn(move || {
        let stdin = io::stdin();
        for line in stdin.lock().lines() {
            if let Some(new_ratio) = line.ok().and_then(|l| l.parse().ok()) {
                *ratio_mut.lock().unwrap() = new_ratio;
            }
        }
    });

    let out_rate = source.sample_rate();
    let mut sink: pulse::Sink<[f32; 2]> = pulse::sink("blarp", out_rate).unwrap();
    for frame in source {
        sink.write_frame(frame).unwrap();
    }
}
