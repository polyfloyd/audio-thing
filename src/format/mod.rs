use crate::audio::*;
use id3;
use std::io::{Read, Seek};
use std::*;

pub mod flac;
pub mod mp3;
pub mod wave;

#[derive(Debug)]
pub enum Format {
    Flac,
    Mp3,
    Wave,
    Unknown,
}

pub fn detect_format(path: &path::Path) -> Result<Format, io::Error> {
    let mut buf = [0; 512];
    let mut file = fs::File::open(path)?;
    let nread = file.read(&mut buf)?;
    file.seek(io::SeekFrom::Start(0))?;

    let header = &buf[..nread];
    if header.starts_with(flac::MAGIC) {
        Ok(Format::Flac)
    } else if mp3::magic().is_match(&header) {
        // Not so fast, the ID3 header can also be slapped on a FLAC file!
        if path.extension().map(|ext| ext.to_string_lossy()) == Some(borrow::Cow::Borrowed("flac"))
        {
            Ok(Format::Flac)
        } else {
            Ok(Format::Mp3)
        }
    } else if wave::magic().is_match(&header) {
        Ok(Format::Wave)
    } else {
        Ok(Format::Unknown)
    }
}

pub fn decode_metadata_file<P>(path: P) -> Result<Metadata, Error>
where
    P: AsRef<path::Path>,
{
    let p = path.as_ref();
    let file = fs::File::open(p)?;
    match detect_format(p)? {
        Format::Flac => Ok(flac::decode(file)?.1),
        Format::Mp3 => Ok(mp3::decode_metadata(file)?),
        Format::Wave => Ok(wave::decode(file)?.1),
        Format::Unknown => Err(Error::Unsupported),
    }
}

pub fn decode_file<P>(path: P) -> Result<(dynam::Audio, Metadata), Error>
where
    P: AsRef<path::Path>,
{
    let p = path.as_ref();
    let file = fs::File::open(p)?;
    match detect_format(p)? {
        Format::Flac => Ok(flac::decode(file)?),
        Format::Mp3 => Ok(mp3::decode(file)?),
        Format::Wave => Ok(wave::decode(file)?),
        Format::Unknown => Err(Error::Unsupported),
    }
}

pub struct Metadata {
    pub sample_rate: u32,
    pub num_samples: Option<u64>,
    pub tag: Option<id3::Tag>,
}

#[derive(Debug, Error)]
pub enum Error {
    /// Format unsopported.
    Unsupported,
    IO(io::Error),
    Flac(flac::Error),
    Mp3(mp3::Error),
    Wave(wave::Error),
}
