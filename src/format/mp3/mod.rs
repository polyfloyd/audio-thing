use std::*;
use std::collections::HashMap;
use id3;
use regex::bytes;
use liblame_sys::*;
use sample;
use ::audio::*;
use ::format;

mod index;
use self::index::FrameIndex;


/// This is the absolute maximum number of samples that can be contained in a single frame.
const MAX_FRAME_SIZE: usize = 1152;
const MAX_FRAME_BYTES: usize = 1348;


pub fn magic() -> &'static bytes::Regex {
    lazy_static! {
        static ref MAGIC: bytes::Regex = bytes::Regex::new(r"^ID3").unwrap();
    }
    &MAGIC
}


pub fn decode<R>(mut input: R) -> Result<(dyn::Audio, format::Metadata), Error>
    where R: io::Read + io::Seek + 'static {
    let id3_tag = {
        let mut buf = [0; 3];
        input.read_exact(&mut buf)?;
        input.seek(io::SeekFrom::Start(0))?;
        if &buf == b"ID3" {
            Some(id3::Tag::read_from(&mut input)?)
        } else {
            None
        }
    };

    let frame_index = FrameIndex::new(&mut input)?;
    input.seek(io::SeekFrom::Start(frame_index.frames[0].offset))?;

    unsafe {
        let hip: hip_t = hip_decode_init();
        if hip.is_null() {
            return Err(Error::ConstructionFailed);
        }
        hip_set_debugf(hip, Some(debug_cb));
        hip_set_msgf(hip, Some(msg_cb));
        hip_set_errorf(hip, Some(error_cb));

        let mut mp3_data = mem::zeroed();
        let mut enc_delay = 0;
        let mut enc_padding = 0;
        let mut buf_left = [0; MAX_FRAME_SIZE];
        let mut buf_right = [0; MAX_FRAME_SIZE];

        let mut rs = 0;
        while rs == 0 {
            let mut read_buf = [0; MAX_FRAME_BYTES];
            let num_read = input.read(&mut read_buf)?;
            rs = hip_decode1_headersB(
                hip,
                read_buf.as_mut_ptr(),
                num_read,
                buf_left.as_mut_ptr(),
                buf_right.as_mut_ptr(),
                &mut mp3_data,
                &mut enc_delay,
                &mut enc_padding,
            );
        }
        if rs == -1 {
            hip_decode_exit(hip);
            return Err(Error::Lame(rs));
        }
        let decode_count = rs;
        assert_eq!(1, mp3_data.header_parsed);
        assert_eq!(MAX_FRAME_SIZE, mp3_data.framesize as usize);

        let sample_rate = mp3_data.samplerate as u32;
        let num_channels = mp3_data.stereo as u32;
        let num_samples = frame_index.frames.last()
            .map(|frame| frame.sample_offset + frame.num_samples as u64)
            .ok_or(Error::Unsupported)?;

        let meta = format::Metadata {
            sample_rate: sample_rate,
            num_samples: Some(num_samples),
            tags: id3_tag.map(|tag| format::tags_from_id3(tag))
                .unwrap_or_else(HashMap::new),
        };
        macro_rules! dyn_type {
            ($dyn:path) => {
                $dyn(Box::from(Decoder {
                    input: input,
                    input_buf: [0; MAX_FRAME_BYTES],
                    hip: hip,
                    index: frame_index,
                    sample_rate: sample_rate,
                    num_samples: num_samples,
                    buffers: [buf_left, buf_right],
                    next_frame: 0,
                    next_sample: 0,
                    samples_available: decode_count as usize,
                    _f: marker::PhantomData,
                })).into()
            }
        }
        Ok((match num_channels {
            1 => dyn_type!(dyn::Seek::MonoI16),
            2 => dyn_type!(dyn::Seek::StereoI16),
            _ => unreachable!(), // LAME's interface does not allow this.
        }, meta))
    }
}


struct Decoder<F, R>
    where F: sample::Frame<Sample=i16>,
          R: io::Read + io::Seek + 'static {
    input: R,
    input_buf: [u8; MAX_FRAME_BYTES],
    hip: hip_t,
    index: FrameIndex,
    sample_rate: u32,
    num_samples: u64,

    buffers: [[i16; MAX_FRAME_SIZE]; 2],
    next_frame: usize,
    next_sample: usize,
    samples_available: usize,

    _f: marker::PhantomData<F>,
}

unsafe impl<F, R> Send for Decoder<F, R>
    where F: sample::Frame<Sample=i16>,
          R: io::Read + io::Seek + 'static { }

impl<F, R> iter::Iterator for Decoder<F, R>
    where F: sample::Frame<Sample=i16>,
          R: io::Read + io::Seek + 'static {
    type Item = F;
    fn next(&mut self) -> Option<Self::Item> {
        let mut num_read = 0;
        while self.next_sample >= self.samples_available {
            unsafe {
                let rs = hip_decode1(
                    self.hip,
                    self.input_buf.as_mut_ptr(),
                    num_read,
                    self.buffers[0].as_mut_ptr(),
                    self.buffers[1].as_mut_ptr(),
                );
                match rs {
                    0 => {
                        if self.next_frame >= self.index.frames.len() {
                            return None;
                        }
                        let frame = &self.index.frames[self.next_frame];
                        if let Err(err) = self.input.seek(io::SeekFrom::Start(frame.offset)) {
                            error!("{}", err);
                            return None;
                        }
                        num_read = match self.input.read(&mut self.input_buf[..frame.length as usize]) {
                            Ok(nr) if nr == 0 => return None,
                            Ok(nr) => nr,
                            Err(err) => {
                                error!("{}", err);
                                return None;
                            },
                        };
                    },
                    code if code < 0 => {
                        error!("Error decoding next frame: {}", Error::Lame(code));
                        return None;
                    },
                    decode_count => {
                        self.next_frame += 1;
                        self.next_sample = 0;
                        self.samples_available = decode_count as usize;
                    },
                };
            }
        }

        let frame = F::from_fn(|ch| self.buffers[ch][self.next_sample]);
        self.next_sample += 1;
        Some(frame)
    }
}

impl<F, R> Source for Decoder<F, R>
    where F: sample::Frame<Sample=i16>,
          R: io::Read + io::Seek + 'static {
    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
}

impl<F, R> Seekable for Decoder<F, R>
    where F: sample::Frame<Sample=i16>,
          R: io::Read + io::Seek + 'static {
    fn seek(&mut self, pos: io::SeekFrom) -> Result<u64, SeekError> {
        let abs_pos = match pos {
            io::SeekFrom::Start(i) => i,
            io::SeekFrom::End(i) => (self.length() + i as u64),
            io::SeekFrom::Current(i) => (self.current_position() + i as u64),
        };
        let i = self.index.frame_for_sample(abs_pos)
            .ok_or(SeekError::OutofRange { pos: abs_pos as i64, size: self.length() })?;
        self.next_frame = i;
        self.next_sample = abs_pos as usize - self.index.frames[i].sample_offset as usize;
        self.samples_available = 0;
        assert!(self.next_frame < self.index.frames.len());
        assert!(self.next_sample < MAX_FRAME_SIZE);
        Ok(abs_pos)
    }

    fn length(&self) -> u64 {
        self.num_samples
    }

    fn current_position(&self) -> u64 {
        if self.next_frame == 0 {
            return 0;
        }
        self.index.frames[self.next_frame - 1].sample_offset + self.next_sample as u64
    }
}

impl<F, R> Seek for Decoder<F, R>
    where F: sample::Frame<Sample=i16>,
          R: io::Read + io::Seek + 'static { }

impl<F, R> Drop for Decoder<F, R>
    where F: sample::Frame<Sample=i16>,
          R: io::Read + io::Seek + 'static {
    fn drop(&mut self) {
        unsafe {
            hip_decode_exit(self.hip);
        }
    }
}


unsafe extern "C" fn debug_cb(format: *const os::raw::c_char, ap: *mut __va_list_tag) {
    debug!("{}", VaFormatter(format, ap));
}

unsafe extern "C" fn msg_cb(format: *const os::raw::c_char, ap: *mut __va_list_tag) {
    info!("{}", VaFormatter(format, ap));
}

unsafe extern "C" fn error_cb(format: *const os::raw::c_char, ap: *mut __va_list_tag) {
    error!("{}", VaFormatter(format, ap));
}

struct VaFormatter(*const os::raw::c_char, *mut __va_list_tag);

impl fmt::Display for VaFormatter {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        unsafe {
            let cstr = ffi::CStr::from_ptr(self.0);
            // A buffer two times the format should be enough in most cases.
            let mut buf = vec![0u8; cstr.to_bytes().len() * 2];
            vsnprintf(buf.as_mut_ptr() as *mut i8, buf.len(), self.0, self.1);
            write!(f, "{}", String::from_utf8_lossy(&*buf))
        }
    }
}


#[derive(Debug)]
pub enum Error {
    IO(io::Error),
    ID3(id3::Error),
    Index(index::Error),
    Lame(i32),
    ConstructionFailed,
    Unsupported,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::IO(ref err) => {
                write!(f, "IO: {}", err)
            },
            Error::ID3(ref err) => {
                write!(f, "ID3: {}", err)
            },
            Error::Index(ref err) => {
                write!(f, "Index: {}", err)
            },
            Error::Lame(code) => {
                let msg = match code {
                    0 => "okay",
                    -1 => "generic error",
                    -10 => "no memory",
                    -11 => "bad bitrate",
                    -12 => "bad sample frequency",
                    -13 => "internal error",
                    -80 => "read error",
                    -81 => "write error",
                    -82 => "file too large",
                    _ => "unknown",
                };
                write!(f, "Lame error: {}", msg)
            },
            Error::ConstructionFailed => {
                write!(f, "Failed to construct decoder")
            },
            Error::Unsupported => {
                write!(f, "Unsupported")
            },
        }
    }
}

impl error::Error for Error {
    fn description(&self) -> &str {
        "MP3 error"
    }

    fn cause(&self) -> Option<&error::Error> {
        match *self {
            Error::IO(ref err) => Some(err),
            Error::ID3(ref err) => Some(err),
            Error::Index(ref err) => Some(err),
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

impl From<index::Error> for Error {
    fn from(err: index::Error) -> Error {
        Error::Index(err)
    }
}
