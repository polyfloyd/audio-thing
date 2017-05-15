use std::*;
use std::io::{Read, Seek};
use id3;
use ::audio::*;

pub mod flac;
pub mod mp3;
pub mod wave;

pub fn decode_metadata_file<P>(path: P) -> Result<Metadata, Error>
    where P: AsRef<path::Path> {
    let mut buf = [0; 512];
    let mut file = fs::File::open(path)?;
    let nread = file.read(&mut buf)?;
    file.seek(io::SeekFrom::Start(0))?;

    let header = &buf[..nread];
    if header.starts_with(flac::MAGIC) {
        return Ok(flac::decode(file)?.1);
    } else if mp3::magic().is_match(&header) {
        return Ok(mp3::decode_metadata(file)?);
    } else if wave::magic().is_match(&header) {
        return Ok(wave::decode(file)?.1);
    }

    Err(Error::Unsupported)
}

pub fn decode_file<P>(path: P) -> Result<(dyn::Audio, Metadata), Error>
    where P: AsRef<path::Path> {
    let mut buf = [0; 512];
    let mut file = fs::File::open(path)?;
    let nread = file.read(&mut buf)?;
    file.seek(io::SeekFrom::Start(0))?;

    let header = &buf[..nread];
    if header.starts_with(flac::MAGIC) {
        return Ok(flac::decode(file)?);
    } else if mp3::magic().is_match(&header) {
        return Ok(mp3::decode(file)?);
    } else if wave::magic().is_match(&header) {
        return Ok(wave::decode(file)?);
    }

    Err(Error::Unsupported)
}


pub struct Metadata {
    pub sample_rate: u32,
    pub num_samples: Option<u64>,
    pub tag: Option<id3::Tag>
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
