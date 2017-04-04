use std::*;
use std::ops::DerefMut;
use libflac_sys::*;
use sample::{self, I24};
use ::audio::*;


struct Decoder<F, R>
    where F: sample::Frame,
          F::Sample: DecodeSample,
          R: io::Read + io::Seek + SeekExt {
    decoder: *mut FLAC__StreamDecoder,
    cb_data: Box<DecoderCallbackData<R>>,

    /// The index of the next read sample in the current block.
    current_sample: usize,
    /// The absolute position in the stream.
    abs_position: u64,
    sample_rate: u32,

    _f: marker::PhantomData<F>,
}

struct DecoderCallbackData<R> {
    input: R,
    current_block: Option<Block>,
}

pub fn open(filename: &path::Path) -> Result<dyn::Audio, LibFlacError> {
    debug!("opening {} for reading", filename.to_string_lossy());
    decode(fs::File::open(filename)?)
}

pub fn decode<R>(input: R) -> Result<dyn::Audio, LibFlacError>
    where R: io::Read + io::Seek + SeekExt + 'static {
    unsafe {
        let decoder = FLAC__stream_decoder_new();
        if decoder.is_null() {
            return Err(LibFlacError::ConstructionFailed);
        }

        let mut cb_data = Box::new(DecoderCallbackData{
            input: input,
            current_block: None,
        });

        let init_status = FLAC__stream_decoder_init_stream(
            decoder,
            Some(read_cb::<R>),
            Some(seek_cb::<R>),
            Some(tell_cb::<R>),
            Some(length_cb::<R>),
            Some(eof_cb::<R>),
            Some(write_cb::<R>),
            None, // Metadata
            Some(error_cb),
            cb_data.deref_mut() as *mut _ as _,
        );
        if init_status != FLAC__StreamDecoderInitStatus::FLAC__STREAM_DECODER_INIT_STATUS_OK {
            FLAC__stream_decoder_delete(decoder);
            return Err(LibFlacError::InitFailed(init_status));
        }

        if FLAC__stream_decoder_process_until_end_of_metadata(decoder) != 1 {
            let state = FLAC__stream_decoder_get_state(decoder);
            FLAC__stream_decoder_delete(decoder);
            return Err(LibFlacError::BadState(state));
        }
        if FLAC__stream_decoder_process_single(decoder) != 1 {
            let state = FLAC__stream_decoder_get_state(decoder);
            FLAC__stream_decoder_delete(decoder);
            return Err(LibFlacError::BadState(state));
        }

        let num_channels = FLAC__stream_decoder_get_channels(decoder);
        let sample_size = FLAC__stream_decoder_get_bits_per_sample(decoder);
        let sample_rate = FLAC__stream_decoder_get_sample_rate(decoder);
        let length = FLAC__stream_decoder_get_total_samples(decoder);
        if length == 0 {
            debug!("got FLAC stream info: {} channels, {} bits, {} hz, length unknown", num_channels, sample_size, sample_rate);
        } else {
            debug!("got FLAC stream info: {} channels, {} bits, {} hz, {} samples", num_channels, sample_size, sample_rate, length);
        }

        macro_rules! dyn_type {
            ($dyn:path) => {
                $dyn(Box::from(Decoder {
                    decoder: decoder,
                    cb_data: cb_data,
                    current_sample: 0,
                    abs_position: 0,
                    sample_rate: sample_rate,
                    _f: marker::PhantomData,
                })).into()
            }
        }
        Ok(match (length != 0, num_channels, sample_size) {
            (false, 1, 8)  => dyn_type!(dyn::Source::MonoI8),
            (false, 1, 16) => dyn_type!(dyn::Source::MonoI16),
            (false, 1, 24) => dyn_type!(dyn::Source::MonoI24),
            (false, 2, 8) => dyn_type!(dyn::Source::StereoI8),
            (false, 2, 16) => dyn_type!(dyn::Source::StereoI16),
            (false, 2, 24) => dyn_type!(dyn::Source::StereoI24),
            (true, 1, 8) => dyn_type!(dyn::Seek::MonoI8),
            (true, 1, 16) => dyn_type!(dyn::Seek::MonoI16),
            (true, 1, 24) => dyn_type!(dyn::Seek::MonoI24),
            (true, 2, 8) => dyn_type!(dyn::Seek::StereoI8),
            (true, 2, 16) => dyn_type!(dyn::Seek::StereoI16),
            (true, 2, 24) => dyn_type!(dyn::Seek::StereoI24),
            (kl, nc, ss) => return Err(LibFlacError::Unimplemented {
                known_length: length != 0,
                num_channels: nc,
                sample_size: ss,
            }),
        })
    }
}

impl<F, R> iter::Iterator for Decoder<F, R>
    where F: sample::Frame,
          F::Sample: DecodeSample,
          R: io::Read + io::Seek + SeekExt {
    type Item = F;
    fn next(&mut self) -> Option<Self::Item> {
        if self.cb_data.current_block.is_none() {
            self.current_sample = 0;
            unsafe {
                if FLAC__stream_decoder_process_single(self.decoder) != 1 {
                    let state = FLAC__stream_decoder_get_state(self.decoder);
                    if state != FLAC__STREAM_DECODER_END_OF_STREAM {
                        error!("{}", LibFlacError::BadState(state));
                    }
                    return None;
                }
            }
            assert!(self.cb_data.current_block.is_some());
        }

        let (frame, block_consumed) = {
            let block = self.cb_data.current_block.as_ref().unwrap();
            let frame = {
                let cs = self.current_sample;
                F::from_fn(|ch| {
                    F::Sample::decode(block.data[ch][cs])
                })
            };
            self.current_sample += 1;
            (frame, self.current_sample == block.data[0].len())
        };
        if block_consumed {
            self.cb_data.current_block = None;
        }

        self.abs_position += 1;
        Some(frame)
    }
}

impl<F, R> Source for Decoder<F, R>
    where F: sample::Frame,
          F::Sample: DecodeSample,
          R: io::Read + io::Seek + SeekExt {
    fn sample_rate(&self) -> u32 { self.sample_rate }
}

impl<F, R> Seekable for Decoder<F, R>
    where F: sample::Frame,
          F::Sample: DecodeSample,
          R: io::Read + io::Seek + SeekExt {
    fn seek(&mut self, pos: io::SeekFrom) -> Result<u64, SeekError> {
        let abs_pos = match pos {
            io::SeekFrom::Start(i) => i as i64,
            io::SeekFrom::End(i) => (self.length() as i64 + i),
            io::SeekFrom::Current(i) => (self.current_position() as i64 + i),
        };
        if abs_pos < 0 || abs_pos >= self.length() as i64 {
            return Err(SeekError::OutofRange{
                pos: abs_pos,
                size: self.length(),
            })
        }
        unsafe {
            if FLAC__stream_decoder_seek_absolute(self.decoder, abs_pos as u64) != 1 {
                let state = FLAC__stream_decoder_get_state(self.decoder);
                return Err(SeekError::Other(Box::from(LibFlacError::BadState(state))));
            }
        }
        self.current_sample = 0;
        self.abs_position = abs_pos as u64;
        Ok(self.abs_position)
    }

    fn length(&self) -> u64 {
        let total_samples = unsafe {
            FLAC__stream_decoder_get_total_samples(self.decoder)
        };
        assert_ne!(0, total_samples);
        total_samples
    }

    fn current_position(&self) -> u64 {
        self.abs_position
    }
}

impl<F, R> Seek for Decoder<F, R>
    where F: sample::Frame,
          F::Sample: DecodeSample,
          R: io::Read + io::Seek + SeekExt { }

unsafe impl<F, R> Send for Decoder<F, R>
    where F: sample::Frame,
          F::Sample: DecodeSample,
          R: io::Read + io::Seek + SeekExt { }

impl<F, R> Drop for Decoder<F, R>
    where F: sample::Frame,
          F::Sample: DecodeSample,
          R: io::Read + io::Seek + SeekExt {
    fn drop(&mut self) {
        unsafe {
            FLAC__stream_decoder_delete(self.decoder);
        }
    }
}


trait DecodeSample {
    fn decode(s: i32) -> Self;
}

impl DecodeSample for i8 {
    fn decode(s: i32) -> i8 { s as i8 }
}

impl DecodeSample for i16 {
    fn decode(s: i32) -> i16 { s as i16 }
}

impl DecodeSample for I24 {
    fn decode(s: i32) -> I24 { I24::new_unchecked(s) }
}


struct Block {
    data: Vec<Vec<i32>>,
}


unsafe extern "C" fn error_cb(_: *const FLAC__StreamDecoder, status: FLAC__StreamDecoderErrorStatus, _: *mut os::raw::c_void) {
    error!("FLAC error callback called: {:?}", status);
}

unsafe extern "C" fn write_cb<R>(_: *const FLAC__StreamDecoder, frame: *const FLAC__Frame, buffer: *const *const FLAC__int32, client_data: *mut os::raw::c_void) -> FLAC__StreamDecoderWriteStatus
    where R: io::Read + io::Seek {
    let data = (client_data as *mut DecoderCallbackData<R>).as_mut().unwrap();

    let fr = frame.as_ref().unwrap();
    let block_data = (0..fr.header.channels)
        .map(|ch| {
            let chan_base_ptr = *buffer.offset(ch as isize);
            (0..fr.header.blocksize)
                .map(|i| *chan_base_ptr.offset(i as isize))
                .collect()
        })
        .collect();
    data.current_block = Some(Block{ data: block_data });
    FLAC__StreamDecoderWriteStatus::FLAC__STREAM_DECODER_WRITE_STATUS_CONTINUE
}

unsafe extern "C" fn read_cb<R>(_: *const FLAC__StreamDecoder, buffer: *mut u8, bytes: *mut usize, client_data: *mut os::raw::c_void) -> FLAC__StreamDecoderReadStatus
    where R: io::Read {
    let mut data = (client_data as *mut DecoderCallbackData<R>).as_mut().unwrap();

    let mut buf = slice::from_raw_parts_mut(buffer, *bytes);
    *bytes = match data.input.read(&mut buf) {
        Ok(read) => read,
        Err(_) => return FLAC__StreamDecoderReadStatus::FLAC__STREAM_DECODER_READ_STATUS_ABORT,
    };
    if *bytes == 0 {
        FLAC__StreamDecoderReadStatus::FLAC__STREAM_DECODER_READ_STATUS_END_OF_STREAM
    } else {
        FLAC__StreamDecoderReadStatus::FLAC__STREAM_DECODER_READ_STATUS_CONTINUE
    }
}

unsafe extern "C" fn seek_cb<R>(_: *const FLAC__StreamDecoder, absolute_byte_offset: u64, client_data: *mut os::raw::c_void) -> FLAC__StreamDecoderSeekStatus
    where R: io::Read + io::Seek {
    let mut data = (client_data as *mut DecoderCallbackData<R>).as_mut().unwrap();
    match data.input.seek(io::SeekFrom::Start(absolute_byte_offset)) {
        Ok(_) => FLAC__StreamDecoderSeekStatus::FLAC__STREAM_DECODER_SEEK_STATUS_OK,
        Err(_) => FLAC__StreamDecoderSeekStatus::FLAC__STREAM_DECODER_SEEK_STATUS_ERROR,
    }
}

unsafe extern "C" fn tell_cb<R>(_: *const FLAC__StreamDecoder, absolute_byte_offset: *mut u64, client_data: *mut os::raw::c_void) -> FLAC__StreamDecoderTellStatus
    where R: io::Read + io::Seek + SeekExt {
    let mut data = (client_data as *mut DecoderCallbackData<R>).as_mut().unwrap();
    if let Ok(pos) = data.input.tell() {
        *absolute_byte_offset = pos;
        return FLAC__StreamDecoderTellStatus::FLAC__STREAM_DECODER_TELL_STATUS_OK;
    }
    FLAC__StreamDecoderTellStatus::FLAC__STREAM_DECODER_TELL_STATUS_ERROR
}

unsafe extern "C" fn length_cb<R>(_: *const FLAC__StreamDecoder, stream_length: *mut u64, client_data: *mut os::raw::c_void) -> FLAC__StreamDecoderLengthStatus
    where R: io::Read + io::Seek + SeekExt {
    let mut data = (client_data as *mut DecoderCallbackData<R>).as_mut().unwrap();
    if let Ok(pos) = data.input.length() {
        *stream_length = pos;
        return FLAC__StreamDecoderLengthStatus::FLAC__STREAM_DECODER_LENGTH_STATUS_OK;
    }
    FLAC__StreamDecoderLengthStatus::FLAC__STREAM_DECODER_LENGTH_STATUS_ERROR
}

unsafe extern "C" fn eof_cb<R>(_: *const FLAC__StreamDecoder, client_data: *mut os::raw::c_void) -> i32
    where R: io::Read + io::Seek + SeekExt {
    let mut data = (client_data as *mut DecoderCallbackData<R>).as_mut().unwrap();
    if data.input.at_eof() {
        1
    } else {
        0
    }
}


pub trait SeekExt: io::Read + io::Seek {
    fn length(&mut self) -> Result<u64, ()>;

    fn at_eof(&mut self) -> bool {
        self.tell()
            .and_then(|pos| self.length().map(|len| pos >= len))
            .unwrap_or(true)
    }

    fn tell(&mut self) -> Result<u64, ()> {
        match self.seek(io::SeekFrom::Current(0)) {
            Ok(pos) => Ok(pos),
            Err(_) => return Err(()),
        }
    }
}

impl SeekExt for fs::File {
    fn length(&mut self) -> Result<u64, ()> {
        match self.metadata() {
            Ok(meta) => Ok(meta.len()),
            Err(_) => Err(()),
        }
    }
}


#[derive(Debug)]
pub enum LibFlacError {
    IO(io::Error),
    ConstructionFailed,
    InitFailed(FLAC__StreamDecoderInitStatus),
    BadState(FLAC__StreamDecoderState),
    Unimplemented{
        known_length: bool,
        num_channels: u32,
        sample_size: u32,
    },
}

impl fmt::Display for LibFlacError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            LibFlacError::IO(ref err) => {
                write!(f, "IO: {}", err)
            },
            LibFlacError::ConstructionFailed => {
                write!(f, "Failed to construct decoder")
            },
            LibFlacError::InitFailed(status) => unsafe {
                let s = FLAC__StreamDecoderInitStatusString.offset(status as isize);
                let errstr = ffi::CStr::from_ptr(*s);
                write!(f, "Flac init failed: {}", errstr.to_str().unwrap())
            },
            LibFlacError::BadState(state) => unsafe {
                let s = FLAC__StreamDecoderStateString.offset(state as isize);
                let errstr = ffi::CStr::from_ptr(*s);
                write!(f, "Flac bad state: {}", errstr.to_str().unwrap())
            },
            LibFlacError::Unimplemented{ known_length: kl, num_channels: nc, sample_size: ss } => {
                write!(f, "Flac format not implemented: {} channels, {} bits, finite: {}", nc, ss, kl)
            },
        }
    }
}

impl error::Error for LibFlacError {
    fn description(&self) -> &str {
        "Flac error"
    }

    fn cause(&self) -> Option<&error::Error> {
        match *self {
            LibFlacError::IO(ref err) => Some(err),
            _ => None,
        }
    }
}

impl From<io::Error> for LibFlacError {
    fn from(err: io::Error) -> LibFlacError {
        LibFlacError::IO(err)
    }
}
