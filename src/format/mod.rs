use std::*;
use std::io::{Read, Seek};
use id3;
use ::audio::*;

pub mod flac;
pub mod mp3;
pub mod wave;


pub fn decode_file(path: &path::Path) -> Result<(dyn::Audio, Metadata), Error> {
    debug!("opening {} for decoding", path.to_string_lossy());

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


#[derive(Debug)]
pub enum Error {
    Unsupported,
    IO(io::Error),
    Flac(flac::Error),
    Mp3(mp3::Error),
    Wave(wave::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::Unsupported => {
                write!(f, "Format unsupported")
            },
            Error::IO(ref err) => {
                write!(f, "IO: {}", err)
            },
            Error::Flac(ref err) => {
                write!(f, "Flac: {}", err)
            },
            Error::Mp3(ref err) => {
                write!(f, "MP3: {}", err)
            },
            Error::Wave(ref err) => {
                write!(f, "Wave: {}", err)
            },
        }
    }
}

impl error::Error for Error {
    fn description(&self) -> &str {
        "Decoder error"
    }

    fn cause(&self) -> Option<&error::Error> {
        match *self {
            Error::Flac(ref err) => Some(err),
            Error::Mp3(ref err) => Some(err),
            Error::Wave(ref err) => Some(err),
            _ => None,
        }
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Error {
        Error::IO(err)
    }
}

impl From<flac::Error> for Error {
    fn from(err: flac::Error) -> Error {
        Error::Flac(err)
    }
}

impl From<mp3::Error> for Error {
    fn from(err: mp3::Error) -> Error {
        Error::Mp3(err)
    }
}

impl From<wave::Error> for Error {
    fn from(err: wave::Error) -> Error {
        Error::Wave(err)
    }
}
