use std::*;
use flac;
use sample::Sample;
use ::audio;

pub struct Decoder<R: io::Read> {
    iter: flac::stream::Iter<flac::ReadStream<R>, <i16 as flac::SampleSize>::Extended, flac::Stream<flac::ReadStream<R>>>,
    info: flac::metadata::StreamInfo,
}

impl<R> Decoder<R>
    where R: io::Read {
    pub fn new(input: R) -> Result<Decoder<R>, Box<error::Error>> {
        let stream = try!(flac::StreamReader::<R>::new(input));
        let info = stream.info();
        let iter = stream.into_iter::<i16>();
        Ok(Decoder {
            iter: iter,
            info: info,
        })
    }
}

impl<R> iter::Iterator for Decoder<R>
    where R: io::Read {
    type Item = [i16; 2];
    fn next(&mut self) -> Option<Self::Item> {
        let flac_channels = self.info.channels as usize;
        let mut flac_frame: Vec<i16> = Vec::with_capacity(flac_channels);
        for _ in 0..flac_channels {
            flac_frame.push(match self.iter.next() {
                Some(s) => s,
                None    => return None,
            });
        }

        debug_assert_eq!(flac_channels, flac_frame.len());
        let out_frame = match flac_channels {
            2 => [flac_frame[0].to_sample(), flac_frame[1].to_sample()],
            _ => unimplemented!(),
        };
        Some(out_frame)
    }
}

impl<R> audio::Source for Decoder<R>
    where R: io::Read {
    fn sample_rate(&self) -> u32 {
        self.info.sample_rate
    }
}

impl<R> audio::Seekable for Decoder<R>
    where R: io::Read {
    fn seek(&mut self, pos: io::SeekFrom) -> Result<(), Box<error::Error>> {
        unimplemented!();
    }

    fn length(&self) -> u64 {
        unimplemented!();
    }

    fn position(&self) -> u64 {
        unimplemented!();
    }
}

impl<R> audio::Seek for Decoder<R>
    where R: io::Read { }
