use std::*;
use std::ops::{Deref, DerefMut};
use sample;
use ::audio::*;

mod libflac {
    #![allow(dead_code, non_snake_case, non_camel_case_types, non_upper_case_globals, improper_ctypes)]
    include!(concat!(env!("OUT_DIR"), "/libflac.rs"));
}
use self::libflac::*;


struct Decoder<F>
    where F: sample::Frame,
          F::Sample: DecodeSample {
    decoder: *mut FLAC__StreamDecoder,
    current_block: Box<Option<Block>>,
    /// The index of the next read sample in the current block.
    current_sample: usize,
    /// The absolute position in the stream.
    abs_position: u64,
    sample_rate: u32,

    _f: marker::PhantomData<F>,
}

pub fn open(filename: &str) -> dyn::Audio {
    unsafe {
        let decoder = FLAC__stream_decoder_new();
        assert!(!decoder.is_null());

        let mut block = Box::new(None);

        let ffi_filename = ffi::CString::new(filename).unwrap();
        let status = FLAC__stream_decoder_init_file(
            decoder,
            ffi_filename.as_ptr(),
            Some(write_cb),
            None,
            Some(error_cb),
            block.deref_mut() as *mut Option<Block> as _,
        );
        assert_eq!(status, FLAC__StreamDecoderInitStatus::FLAC__STREAM_DECODER_INIT_STATUS_OK);

        let ok = FLAC__stream_decoder_process_until_end_of_metadata(decoder);
        assert_eq!(1, ok);
        let ok = FLAC__stream_decoder_process_single(decoder);
        assert_eq!(1, ok);

        let num_channels = FLAC__stream_decoder_get_channels(decoder);
        let sample_size = FLAC__stream_decoder_get_bits_per_sample(decoder);
        let sample_rate = FLAC__stream_decoder_get_sample_rate(decoder);
        let known_length = FLAC__stream_decoder_get_total_samples(decoder) != 0;

        match (known_length, num_channels, sample_size) {
            (true, 2, 16) => dyn::Seek::StereoI16(Box::from(Decoder {
                decoder: decoder,
                current_block: block,
                current_sample: 0,
                abs_position: 0,
                sample_rate: sample_rate,
                _f: marker::PhantomData,
            })).into(),
            _ => unimplemented!(),
        }
    }
}

impl<F> iter::Iterator for Decoder<F>
    where F: sample::Frame,
          F::Sample: DecodeSample {
    type Item = F;
    fn next(&mut self) -> Option<Self::Item> {
        if self.current_block.is_none() {
            return None;
        }

        let frame = {
            let ref data = self.current_block.deref().as_ref().unwrap().data;
            F::from_fn(|ch| {
                F::Sample::decode(data[ch][self.current_sample])
            })
        };

        self.current_sample += 1;
        if self.current_sample == self.current_block.deref().as_ref().unwrap().data[0].len() {
            self.current_sample = 0;
            // Load the next block.
            unsafe {
                let ok = FLAC__stream_decoder_process_single(self.decoder);
                assert_eq!(1, ok);
                let state = FLAC__stream_decoder_get_state(self.decoder);
                if state == FLAC__STREAM_DECODER_END_OF_STREAM {
                    *self.current_block.deref_mut() = None;
                }
            }
        }

        self.abs_position += 1;
        Some(frame)
    }
}

impl<F> Source for Decoder<F>
    where F: sample::Frame,
          F::Sample: DecodeSample {
    fn sample_rate(&self) -> u32 { self.sample_rate }
}

impl<F> Seekable for Decoder<F>
    where F: sample::Frame,
          F::Sample: DecodeSample {
    fn seek(&mut self, pos: io::SeekFrom) -> Result<(), Box<error::Error>> {
        let abs_pos = match pos {
            io::SeekFrom::Start(i) => i,
            io::SeekFrom::End(i) => (self.length() as i64 + i) as u64,
            io::SeekFrom::Current(i) => (self.current_position() as i64 + i) as u64,
        };
        assert!(abs_pos < self.length());
        unsafe {
            let ok = FLAC__stream_decoder_seek_absolute(self.decoder, abs_pos);
            assert_eq!(1, ok);
        }
        self.current_sample = 0;
        self.abs_position = abs_pos;
        Ok(())
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

impl<F> Seek for Decoder<F>
    where F: sample::Frame,
          F::Sample: DecodeSample { }

unsafe impl<F> Send for Decoder<F>
    where F: sample::Frame,
          F::Sample: DecodeSample { }

impl<F> Drop for Decoder<F>
    where F: sample::Frame,
          F::Sample: DecodeSample {
    fn drop(&mut self) {
        unsafe {
            FLAC__stream_decoder_delete(self.decoder);
        }
    }
}


pub trait DecodeSample {
    fn decode(s: i32) -> Self;
}

impl DecodeSample for i16 {
    fn decode(s: i32) -> i16 { s as i16 }
}


struct Block {
    data: Vec<Vec<i32>>,
}


unsafe extern "C" fn write_cb(_: *const FLAC__StreamDecoder, frame: *const FLAC__Frame, buffer: *const *const FLAC__int32, client_data: *mut os::raw::c_void) -> FLAC__StreamDecoderWriteStatus {
    let fr = frame.as_ref().unwrap();
    let data = (0..fr.header.channels)
        .map(|ch| {
            let chan_base_ptr = *buffer.offset(ch as isize);
            (0..fr.header.blocksize)
                .map(|i| *chan_base_ptr.offset(i as isize))
                .collect()
        })
        .collect();
    *(client_data as *mut Option<Block>) = Some(Block{ data: data });
    FLAC__StreamDecoderWriteStatus::FLAC__STREAM_DECODER_WRITE_STATUS_CONTINUE
}

unsafe extern "C" fn error_cb(_: *const FLAC__StreamDecoder, _: FLAC__StreamDecoderErrorStatus, _: *mut os::raw::c_void) { }





//#[derive(Debug)]
//pub struct LibflacError(FLAC__StreamDecoderState);
//
//impl fmt::Display for PulseError {
//    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
//        unsafe {
//            let errstr = ffi::CStr::from_ptr(pa_strerror(self.0 as i32));
//            write!(f, "Pulse error: {}", errstr.to_str().unwrap())
//        }
//    }
//}
//
//impl error::Error for PulseError {
//    fn description(&self) -> &str {
//        unsafe {
//            let errstr = ffi::CStr::from_ptr(pa_strerror(self.0 as i32));
//            errstr.to_str().unwrap()
//        }
//    }
//    fn cause(&self) -> Option<&error::Error> {
//        None
//    }
//}
