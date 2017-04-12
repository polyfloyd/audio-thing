use std::*;
use std::collections::HashMap;
use std::io::{Read, Seek};
use std::iter::FromIterator;
use id3;
use ::audio::*;

pub mod flac;
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
    } else if wave::magic().is_match(&header) {
        return Ok(wave::decode(file)?);
    }

    Err(Error::Unsupported)
}


#[derive(Debug, Clone)]
pub struct Metadata {
    pub sample_rate: u32,
    pub num_samples: Option<u64>,

    /// There seems to be no real standard for music tags, so all other tags read by decoders
    /// should thrown in this hashmap.
    /// Decoders should restrict keys to lowercase alphanumeric characters.
    ///
    /// NOTE: An id3 tag is probably better than this hashmap, so this might change sometime.
    pub tags: HashMap<String, String>,
}

/// Copies the `Metadata` tags from an ID3 tag.
fn tags_from_id3(tag: id3::Tag) -> HashMap<String, String> {
    let t = tag.frames()
        .iter()
        .filter_map(|frame| {
            let key = match frame.id.as_str() {
                "TPE1"|"TPE2" => "artist",
                "TIT2" => "title",
                "TCON" => "genre",
                "COMM" => "comment",
                _ => return None, // TODO: More ids
            };
            let value = match frame.content {
                id3::frame::Content::Text(ref s) => s,
                id3::frame::Content::Link(ref s) => s,
                _ => return None,
            };
            Some((key.to_string(), value.to_string()))
        });
    HashMap::from_iter(t)
}


#[derive(Debug)]
pub enum Error {
    Unsupported,
    IO(io::Error),
    Flac(flac::Error),
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

impl From<wave::Error> for Error {
    fn from(err: wave::Error) -> Error {
        Error::Wave(err)
    }
}
