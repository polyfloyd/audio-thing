use std::*;
use std::collections::HashMap;
use id3;
use regex::bytes;
use liblame_sys::*;
use sample;
use ::audio::*;
use ::format;


const MAX_FRAME_SIZE: usize = 1152;


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
            let mut read_buf = [0; 8192];
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
        let num_samples = match mp3_data.nsamp {
            0 => None,
            n => Some(n),
        };
        let meta = format::Metadata {
            sample_rate: sample_rate,
            num_samples: num_samples,
            tags: id3_tag.map(|tag| format::tags_from_id3(tag))
                .unwrap_or_else(HashMap::new),
        };

        macro_rules! dyn_type {
            ($dyn:path) => {
                $dyn(Box::from(Decoder {
                    input: input,
                    hip: hip,
                    sample_rate: sample_rate,
                    buffers: [buf_left, buf_right],
                    next_sample: 0,
                    samples_available: decode_count as usize,
                    _f: marker::PhantomData,
                })).into()
            }
        }
        Ok((match num_channels {
            1 => dyn_type!(dyn::Source::MonoI16),
            2 => dyn_type!(dyn::Source::StereoI16),
            _ => unreachable!(), // LAME's interface does not allow this.
        }, meta))
    }
}


struct Decoder<F, R>
    where F: sample::Frame<Sample=i16>,
          R: io::Read + io::Seek + 'static {
    input: R,
    hip: hip_t,
    sample_rate: u32,

    buffers: [[i16; MAX_FRAME_SIZE]; 2],
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
        while self.next_sample >= self.samples_available {
            let mut read_buf = [0; 8192];
            let num_read = match self.input.read(&mut read_buf) {
                Ok(nr) if nr == 0 => return None,
                Ok(nr) => nr,
                Err(err) => {
                    error!("{}", err);
                    return None;
                },
            };
            unsafe {
                let rs = hip_decode1(
                    self.hip,
                    read_buf.as_mut_ptr(),
                    num_read,
                    self.buffers[0].as_mut_ptr(),
                    self.buffers[1].as_mut_ptr(),
                );
                match rs {
                    0 => (),
                    code if code < 0 => {
                        error!("Error decoding next frame: {}", Error::Lame(code));
                        return None;
                    },
                    decode_count => {
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
    Lame(i32),
    ConstructionFailed,
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
        }
    }
}

impl error::Error for Error {
    fn description(&self) -> &str {
        "LAME error"
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
