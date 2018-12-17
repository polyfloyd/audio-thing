use crate::audio::*;
use byteorder::{BigEndian, ByteOrder, LittleEndian};
use crate::format;
use id3;
use log::*;
use regex::bytes;
use sample::{self, I24};
use std::*;
use lazy_static::lazy_static;

pub fn magic() -> &'static bytes::Regex {
    lazy_static! {
        static ref MAGIC: bytes::Regex = bytes::Regex::new(r"(?s-u)^RIF(F|X)....WAVE").unwrap();
    }
    &MAGIC
}

#[derive(Copy, Clone, Debug)]
pub enum Endianness {
    Big,
    Little,
}

#[derive(Copy, Clone, Debug)]
enum Format {
    Int,
    Float,
}

pub fn decode<R>(mut input: R) -> Result<(dynam::Audio, format::Metadata), Error>
where
    R: io::Read + io::Seek + Send + 'static,
{
    // Read the file header.
    let mut file_header = [0; 12];
    input.read_exact(&mut file_header)?;
    let endianness = magic()
        .captures(&file_header)
        .and_then(|cap| cap.get(1))
        .and_then(|m| match m.as_bytes() {
            b"F" => Some(Endianness::Little),
            b"X" => Some(Endianness::Big),
            _ => None,
        }).ok_or(Error::FormatError)?;

    struct FmtChunk {
        audio_format: u16,
        num_channels: u16,
        sample_rate: u32,
        block_align: u16,
        sample_size: u16,
    }
    let mut fmt = None;
    let mut id3_tag = None;
    let mut data_range = None;

    // Read all chunks in the file until we reach the end.
    let mut sub_header = [0; 8];
    while input.read_exact(&mut sub_header).is_ok() {
        let sub_size = u64::from(LittleEndian::read_u32(&sub_header[4..8]));
        let sub_data_start = input.seek(io::SeekFrom::Current(0))?;

        match &sub_header[0..4] {
            b"fmt " => {
                let mut buf = [0; 16];
                input.read_exact(&mut buf)?;
                fmt = Some(FmtChunk {
                    audio_format: LittleEndian::read_u16(&buf[0..2]),
                    num_channels: LittleEndian::read_u16(&buf[2..4]),
                    sample_rate: LittleEndian::read_u32(&buf[4..8]),
                    // 8..12 = byte_rate
                    block_align: LittleEndian::read_u16(&buf[12..14]),
                    sample_size: LittleEndian::read_u16(&buf[14..16]),
                });
            }

            b"id3 " => {
                id3_tag = Some(id3::Tag::read_from(&mut input)?);
            }

            b"data" => {
                data_range = Some(sub_data_start..sub_data_start + sub_size);
            }

            // Broadcast Audio Extension Chunk (BWF). Unimplemented.
            b"bext" => (),

            // Some kind of metadata added by Logic Pro. Unimplemented.
            b"LGWV" => (),

            // A padding chunk is used to reserve space for a future chunk so the data chunk does
            // not have to be moved.
            b"PAD " => (),

            id => debug!("unknown chunk id: {}", String::from_utf8_lossy(id)),
        };

        input.seek(io::SeekFrom::Start(sub_data_start + sub_size))?;
    }

    let fmt = fmt.ok_or(Error::FormatError)?;
    let data_range = data_range.ok_or(Error::FormatError)?;
    if fmt.block_align != fmt.num_channels * fmt.sample_size / 8 {
        error!(
            "mismatch: block_align: {}, num_channels * sample_size: {}",
            fmt.block_align,
            fmt.num_channels * fmt.sample_size / 8
        );
        return Err(Error::FormatError);
    }
    let audio_format = match fmt.audio_format {
        1 => Format::Int,
        3 => Format::Float,
        _ => return Err(Error::Unsupported),
    };
    input.seek(io::SeekFrom::Start(data_range.start))?;

    debug!(
        "{} channels, {} bits, {} hz, endianness: {:?}",
        fmt.num_channels, fmt.sample_size, fmt.sample_rate, endianness
    );

    let meta = format::Metadata {
        sample_rate: fmt.sample_rate,
        num_samples: Some(
            (data_range.end - data_range.start) / u64::from(fmt.num_channels * fmt.sample_size / 8),
        ),
        tag: id3_tag,
    };

    macro_rules! dyn_type {
        ($dyn:path, $end:path) => {
            $dyn(Box::from(Decoder::<_, _, $end> {
                input: input,
                data_range: data_range,
                sample_rate: fmt.sample_rate,
                num_channels: fmt.num_channels as usize,
                bytes_per_sample: fmt.sample_size as usize / 8,
                next_sample: 0,
                ph_f: marker::PhantomData,
                ph_b: marker::PhantomData,
            })).into()
        };
    }
    Ok((
        match (fmt.num_channels, fmt.sample_size, audio_format, endianness) {
            (1, 8, Format::Int, Endianness::Little) => dyn_type!(dynam::Seek::MonoU8, LittleEndian),
            (1, 16, Format::Int, Endianness::Little) => dyn_type!(dynam::Seek::MonoI16, LittleEndian),
            (1, 24, Format::Int, Endianness::Little) => dyn_type!(dynam::Seek::MonoI24, LittleEndian),
            (1, 32, Format::Float, Endianness::Little) => {
                dyn_type!(dynam::Seek::MonoF32, LittleEndian)
            }
            (2, 8, Format::Int, Endianness::Little) => dyn_type!(dynam::Seek::StereoU8, LittleEndian),
            (2, 16, Format::Int, Endianness::Little) => {
                dyn_type!(dynam::Seek::StereoI16, LittleEndian)
            }
            (2, 24, Format::Int, Endianness::Little) => {
                dyn_type!(dynam::Seek::StereoI24, LittleEndian)
            }
            (2, 32, Format::Float, Endianness::Little) => {
                dyn_type!(dynam::Seek::StereoF32, LittleEndian)
            }
            (nc, ss, _, end) => {
                return Err(Error::Unimplemented {
                    endianness: end,
                    num_channels: nc,
                    sample_size: ss,
                })
            }
        },
        meta,
    ))
}

struct Decoder<R, F, B>
where
    R: io::Read + io::Seek,
    F: sample::Frame,
    F::Sample: DecodeSample<B>,
    B: ByteOrder,
{
    input: R,
    /// Position of the first PCM byte in the file.
    data_range: ops::Range<u64>,

    sample_rate: u32,
    num_channels: usize,
    bytes_per_sample: usize,

    next_sample: u64,

    ph_f: marker::PhantomData<F>,
    ph_b: marker::PhantomData<B>,
}

impl<R, F, B> iter::Iterator for Decoder<R, F, B>
where
    R: io::Read + io::Seek,
    F: sample::Frame,
    F::Sample: DecodeSample<B>,
    B: ByteOrder,
{
    type Item = F;
    fn next(&mut self) -> Option<Self::Item> {
        let fpos = match self.input.seek(io::SeekFrom::Current(0)) {
            Ok(fpos) => fpos,
            Err(err) => {
                error!("error getting position: {}", err);
                return None;
            }
        };
        if fpos < self.data_range.start || self.data_range.end <= fpos {
            return None;
        }

        let mut buf = vec![0; self.num_channels * self.bytes_per_sample];
        match self.input.read(&mut buf) {
            Ok(nread) if nread != buf.len() => {
                return None;
            }
            Err(err) => {
                error!("error reading sample: {}", err);
                return None;
            }
            _ => (),
        };

        self.next_sample =
            (fpos - self.data_range.start) / (self.num_channels * self.bytes_per_sample) as u64 + 1;

        Some(F::from_fn(|channel| {
            let offset = channel * self.bytes_per_sample;
            F::Sample::decode(&buf[offset..])
        }))
    }
}

impl<R, F, B> Source for Decoder<R, F, B>
where
    R: io::Read + io::Seek,
    F: sample::Frame,
    F::Sample: DecodeSample<B>,
    B: ByteOrder,
{
    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
}

impl<R, F, B> Seekable for Decoder<R, F, B>
where
    R: io::Read + io::Seek,
    F: sample::Frame,
    F::Sample: DecodeSample<B>,
    B: ByteOrder,
{
    fn seek(&mut self, position: u64) -> Result<(), SeekError> {
        if position >= self.length() {
            return Err(SeekError::OutofRange {
                pos: position,
                size: self.length(),
            });
        }

        let fpos =
            self.data_range.start + position * (self.num_channels * self.bytes_per_sample) as u64;
        self.input
            .seek(io::SeekFrom::Start(fpos))
            .map_err(|err| SeekError::Other(Box::from(err)))?;
        self.next_sample = position;
        Ok(())
    }

    fn length(&self) -> u64 {
        (self.data_range.end - self.data_range.start)
            / (self.num_channels * self.bytes_per_sample) as u64
    }

    fn current_position(&self) -> u64 {
        self.next_sample
    }
}

impl<R, F, B> Seek for Decoder<R, F, B>
where
    R: io::Read + io::Seek,
    F: sample::Frame,
    F::Sample: DecodeSample<B>,
    B: ByteOrder,
{}

trait DecodeSample<B>: sample::Sample
where
    B: ByteOrder,
{
    fn decode(buf: &[u8]) -> Self;
}

impl<B> DecodeSample<B> for u8
where
    B: ByteOrder,
{
    fn decode(buf: &[u8]) -> u8 {
        buf[0]
    }
}

impl<B> DecodeSample<B> for i16
where
    B: ByteOrder,
{
    fn decode(buf: &[u8]) -> i16 {
        B::read_i16(buf)
    }
}

impl DecodeSample<BigEndian> for I24 {
    fn decode(buf: &[u8]) -> I24 {
        I24::new_unchecked(i32::from(buf[2]) | i32::from(buf[1]) << 8 | i32::from(buf[0]) << 16)
    }
}

impl DecodeSample<LittleEndian> for I24 {
    fn decode(buf: &[u8]) -> I24 {
        I24::new_unchecked(i32::from(buf[0]) | i32::from(buf[1]) << 8 | i32::from(buf[2]) << 16)
    }
}

impl<B> DecodeSample<B> for f32
where
    B: ByteOrder,
{
    fn decode(buf: &[u8]) -> f32 {
        B::read_f32(buf)
    }
}

#[derive(Debug)]
pub enum Error {
    IO(io::Error),
    ID3(id3::Error),
    FormatError,
    Unimplemented {
        endianness: Endianness,
        num_channels: u16,
        sample_size: u16,
    },
    Unsupported,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::IO(ref err) => write!(f, "IO: {}", err),
            Error::ID3(ref err) => write!(f, "ID3: {}", err),
            Error::FormatError => write!(f, "Format error"),
            Error::Unimplemented {
                endianness,
                num_channels,
                sample_size,
            } => write!(
                f,
                "Wave format not implemented: {} channels, {} bits, endianness: {:?}",
                num_channels, sample_size, endianness,
            ),
            Error::Unsupported => write!(f, "Non PCM formats are unsupported"),
        }
    }
}

impl error::Error for Error {
    fn description(&self) -> &str {
        "Wave error"
    }

    fn cause(&self) -> Option<&error::Error> {
        match *self {
            Error::IO(ref err) => Some(err),
            Error::ID3(ref err) => Some(err),
            _ => None,
        }
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Error {
        Error::IO(err)
    }
}

impl From<id3::Error> for Error {
    fn from(err: id3::Error) -> Error {
        Error::ID3(err)
    }
}

#[cfg(all(test, feature = "unstable"))]
mod benchmarks {
    extern crate test;
    use super::*;

    #[bench]
    fn read_metadata(b: &mut test::Bencher) {
        b.iter(|| {
            let file = fs::File::open("testdata/10s_440hz_i16.wav").unwrap();
            decode(file).unwrap();
        });
    }
}
