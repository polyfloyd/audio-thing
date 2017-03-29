use std::*;
use sample::{I24, U24};


pub enum Source {
    MonoI8(   Box<super::Source<Item=[i8;  1]> + Send>),
    MonoU8(   Box<super::Source<Item=[u8;  1]> + Send>),
    MonoI16(  Box<super::Source<Item=[i16; 1]> + Send>),
    MonoU16(  Box<super::Source<Item=[u16; 1]> + Send>),
    MonoI24(  Box<super::Source<Item=[I24; 1]> + Send>),
    MonoU24(  Box<super::Source<Item=[U24; 1]> + Send>),
    MonoI32(  Box<super::Source<Item=[i32; 1]> + Send>),
    MonoU32(  Box<super::Source<Item=[u32; 1]> + Send>),
    MonoI64(  Box<super::Source<Item=[i64; 1]> + Send>),
    MonoU64(  Box<super::Source<Item=[u64; 1]> + Send>),
    MonoF32(  Box<super::Source<Item=[f32; 1]> + Send>),
    MonoF64(  Box<super::Source<Item=[f64; 1]> + Send>),
    StereoI8( Box<super::Source<Item=[i8;  2]> + Send>),
    StereoU8( Box<super::Source<Item=[u8;  2]> + Send>),
    StereoI24(Box<super::Source<Item=[I24; 2]> + Send>),
    StereoU24(Box<super::Source<Item=[U24; 2]> + Send>),
    StereoI16(Box<super::Source<Item=[i16; 2]> + Send>),
    StereoU16(Box<super::Source<Item=[u16; 2]> + Send>),
    StereoI32(Box<super::Source<Item=[i32; 2]> + Send>),
    StereoU32(Box<super::Source<Item=[u32; 2]> + Send>),
    StereoI64(Box<super::Source<Item=[i64; 2]> + Send>),
    StereoU64(Box<super::Source<Item=[u64; 2]> + Send>),
    StereoF32(Box<super::Source<Item=[f32; 2]> + Send>),
    StereoF64(Box<super::Source<Item=[f64; 2]> + Send>),
}

impl Source {
    pub fn sample_rate(&self) -> u32 {
        match self {
            &Source::MonoI8(ref s) => s.sample_rate(),
            &Source::MonoU8(ref s) => s.sample_rate(),
            &Source::MonoI16(ref s) => s.sample_rate(),
            &Source::MonoU16(ref s) => s.sample_rate(),
            &Source::MonoI24(ref s) => s.sample_rate(),
            &Source::MonoU24(ref s) => s.sample_rate(),
            &Source::MonoI32(ref s) => s.sample_rate(),
            &Source::MonoU32(ref s) => s.sample_rate(),
            &Source::MonoI64(ref s) => s.sample_rate(),
            &Source::MonoU64(ref s) => s.sample_rate(),
            &Source::MonoF32(ref s) => s.sample_rate(),
            &Source::MonoF64(ref s) => s.sample_rate(),
            &Source::StereoI8(ref s) => s.sample_rate(),
            &Source::StereoU8(ref s) => s.sample_rate(),
            &Source::StereoI16(ref s) => s.sample_rate(),
            &Source::StereoU16(ref s) => s.sample_rate(),
            &Source::StereoI24(ref s) => s.sample_rate(),
            &Source::StereoU24(ref s) => s.sample_rate(),
            &Source::StereoI32(ref s) => s.sample_rate(),
            &Source::StereoU32(ref s) => s.sample_rate(),
            &Source::StereoI64(ref s) => s.sample_rate(),
            &Source::StereoU64(ref s) => s.sample_rate(),
            &Source::StereoF32(ref s) => s.sample_rate(),
            &Source::StereoF64(ref s) => s.sample_rate(),
        }
    }
}


pub enum Seek {
    MonoI8(   Box<super::Seek<Item=[i8;  1]> + Send>),
    MonoU8(   Box<super::Seek<Item=[u8;  1]> + Send>),
    MonoI16(  Box<super::Seek<Item=[i16; 1]> + Send>),
    MonoU16(  Box<super::Seek<Item=[u16; 1]> + Send>),
    MonoI24(  Box<super::Seek<Item=[I24; 1]> + Send>),
    MonoU24(  Box<super::Seek<Item=[U24; 1]> + Send>),
    MonoI32(  Box<super::Seek<Item=[i32; 1]> + Send>),
    MonoU32(  Box<super::Seek<Item=[u32; 1]> + Send>),
    MonoI64(  Box<super::Seek<Item=[i64; 1]> + Send>),
    MonoU64(  Box<super::Seek<Item=[u64; 1]> + Send>),
    MonoF32(  Box<super::Seek<Item=[f32; 1]> + Send>),
    MonoF64(  Box<super::Seek<Item=[f64; 1]> + Send>),
    StereoI8( Box<super::Seek<Item=[i8;  2]> + Send>),
    StereoU8( Box<super::Seek<Item=[u8;  2]> + Send>),
    StereoI16(Box<super::Seek<Item=[i16; 2]> + Send>),
    StereoU16(Box<super::Seek<Item=[u16; 2]> + Send>),
    StereoI24(Box<super::Seek<Item=[I24; 2]> + Send>),
    StereoU24(Box<super::Seek<Item=[U24; 2]> + Send>),
    StereoI32(Box<super::Seek<Item=[i32; 2]> + Send>),
    StereoU32(Box<super::Seek<Item=[u32; 2]> + Send>),
    StereoI64(Box<super::Seek<Item=[i64; 2]> + Send>),
    StereoU64(Box<super::Seek<Item=[u64; 2]> + Send>),
    StereoF32(Box<super::Seek<Item=[f32; 2]> + Send>),
    StereoF64(Box<super::Seek<Item=[f64; 2]> + Send>),
}

impl Seek {
    pub fn sample_rate(&self) -> u32 {
        match self {
            &Seek::MonoI8(ref s) => s.sample_rate(),
            &Seek::MonoU8(ref s) => s.sample_rate(),
            &Seek::MonoI16(ref s) => s.sample_rate(),
            &Seek::MonoU16(ref s) => s.sample_rate(),
            &Seek::MonoI24(ref s) => s.sample_rate(),
            &Seek::MonoU24(ref s) => s.sample_rate(),
            &Seek::MonoI32(ref s) => s.sample_rate(),
            &Seek::MonoU32(ref s) => s.sample_rate(),
            &Seek::MonoI64(ref s) => s.sample_rate(),
            &Seek::MonoU64(ref s) => s.sample_rate(),
            &Seek::MonoF32(ref s) => s.sample_rate(),
            &Seek::MonoF64(ref s) => s.sample_rate(),
            &Seek::StereoI8(ref s) => s.sample_rate(),
            &Seek::StereoU8(ref s) => s.sample_rate(),
            &Seek::StereoI16(ref s) => s.sample_rate(),
            &Seek::StereoU16(ref s) => s.sample_rate(),
            &Seek::StereoI24(ref s) => s.sample_rate(),
            &Seek::StereoU24(ref s) => s.sample_rate(),
            &Seek::StereoI32(ref s) => s.sample_rate(),
            &Seek::StereoU32(ref s) => s.sample_rate(),
            &Seek::StereoI64(ref s) => s.sample_rate(),
            &Seek::StereoU64(ref s) => s.sample_rate(),
            &Seek::StereoF32(ref s) => s.sample_rate(),
            &Seek::StereoF64(ref s) => s.sample_rate(),
        }
    }
}

impl<T> From<T> for Seek
    where T: super::Seek<Item=[f64; 2]> + Send + 'static {
    fn from(seek: T) -> Seek { Seek::StereoF64(Box::from(seek)) }
}

impl From<Seek> for Source {
    fn from(seek: Seek) -> Source {
        match seek {
            Seek::MonoI8(s) => Source::MonoI8(Box::new(s)),
            Seek::MonoU8(s) => Source::MonoU8(Box::new(s)),
            Seek::MonoI16(s) => Source::MonoI16(Box::new(s)),
            Seek::MonoU16(s) => Source::MonoU16(Box::new(s)),
            Seek::MonoI24(s) => Source::MonoI24(Box::new(s)),
            Seek::MonoU24(s) => Source::MonoU24(Box::new(s)),
            Seek::MonoI32(s) => Source::MonoI32(Box::new(s)),
            Seek::MonoU32(s) => Source::MonoU32(Box::new(s)),
            Seek::MonoI64(s) => Source::MonoI64(Box::new(s)),
            Seek::MonoU64(s) => Source::MonoU64(Box::new(s)),
            Seek::MonoF32(s) => Source::MonoF32(Box::new(s)),
            Seek::MonoF64(s) => Source::MonoF64(Box::new(s)),
            Seek::StereoI8(s) => Source::StereoI8(Box::new(s)),
            Seek::StereoU8(s) => Source::StereoU8(Box::new(s)),
            Seek::StereoI16(s) => Source::StereoI16(Box::new(s)),
            Seek::StereoU16(s) => Source::StereoU16(Box::new(s)),
            Seek::StereoI24(s) => Source::StereoI24(Box::new(s)),
            Seek::StereoU24(s) => Source::StereoU24(Box::new(s)),
            Seek::StereoI32(s) => Source::StereoI32(Box::new(s)),
            Seek::StereoU32(s) => Source::StereoU32(Box::new(s)),
            Seek::StereoI64(s) => Source::StereoI64(Box::new(s)),
            Seek::StereoU64(s) => Source::StereoU64(Box::new(s)),
            Seek::StereoF32(s) => Source::StereoF32(Box::new(s)),
            Seek::StereoF64(s) => Source::StereoF64(Box::new(s)),
        }
    }
}


pub enum Sink {
    MonoI8(   Box<super::Sink<[i8;  1]>>),
    MonoU8(   Box<super::Sink<[u8;  1]>>),
    MonoI16(  Box<super::Sink<[i16; 1]>>),
    MonoU16(  Box<super::Sink<[u16; 1]>>),
    MonoI24(  Box<super::Sink<[I24; 1]>>),
    MonoU24(  Box<super::Sink<[U24; 1]>>),
    MonoI32(  Box<super::Sink<[i32; 1]>>),
    MonoU32(  Box<super::Sink<[u32; 1]>>),
    MonoI64(  Box<super::Sink<[i64; 1]>>),
    MonoU64(  Box<super::Sink<[u64; 1]>>),
    MonoF32(  Box<super::Sink<[f32; 1]>>),
    MonoF64(  Box<super::Sink<[f64; 1]>>),
    StereoI8( Box<super::Sink<[i8;  2]>>),
    StereoU8( Box<super::Sink<[u8;  2]>>),
    StereoI16(Box<super::Sink<[i16; 2]>>),
    StereoU16(Box<super::Sink<[u16; 2]>>),
    StereoI24(Box<super::Sink<[I24; 2]>>),
    StereoU24(Box<super::Sink<[U24; 2]>>),
    StereoI32(Box<super::Sink<[i32; 2]>>),
    StereoU32(Box<super::Sink<[u32; 2]>>),
    StereoI64(Box<super::Sink<[i64; 2]>>),
    StereoU64(Box<super::Sink<[u64; 2]>>),
    StereoF32(Box<super::Sink<[f32; 2]>>),
    StereoF64(Box<super::Sink<[f64; 2]>>),
}

pub enum Audio {
    Source(Source),
    Seek(Seek),
}

impl Audio {
    pub fn sample_rate(&self) -> u32 {
        match self {
            &Audio::Source(ref s) => s.sample_rate(),
            &Audio::Seek(ref s) => s.sample_rate(),
        }
    }
}

impl From<Source> for Audio {
    fn from(source: Source) -> Audio { Audio::Source(source) }
}

impl From<Seek> for Audio {
    fn from(seek: Seek) -> Audio { Audio::Seek(seek) }
}

impl From<Audio> for Source {
    fn from(aud: Audio) -> Source {
        match aud {
            Audio::Source(s) => s,
            Audio::Seek(s) => s.into(),
        }
    }
}
