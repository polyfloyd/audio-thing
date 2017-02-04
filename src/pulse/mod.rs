use std::*;
use std::io::Read;
use sample;

mod simple;
use self::simple::*;

mod pulse_simple {
    #![allow(dead_code, non_camel_case_types, non_upper_case_globals, improper_ctypes)]
    include!(concat!(env!("OUT_DIR"), "/pulse-simple.rs"));
}
mod pulse_error {
    #![allow(dead_code, non_camel_case_types, non_upper_case_globals, improper_ctypes)]
    include!(concat!(env!("OUT_DIR"), "/pulse-error.rs"));
}
use self::pulse_simple::*;
use self::pulse_error::pa_strerror;


pub struct Source<F: sample::Frame> {
    conn: Connection<F>,
}

impl<F> iter::Iterator for Source<F>
    where F: sample::Frame {
    type Item = F;
    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            debug_assert_eq!(mem::size_of::<F>(), F::n_channels() * mem::size_of::<F::Sample>());
            let mut frame: F = mem::uninitialized();
            let buf = slice::from_raw_parts_mut(mem::transmute::<&F, *mut u8>(&mut frame), mem::size_of::<F>());
            self.conn.read(buf).unwrap();
            Some(frame)
        }
    }
}

pub fn source<F, S>(app_name: &str, rate: u32) -> Result<Source<F>, Box<error::Error>>
    where F: sample::Frame<Sample=S>,
          S: sample::Sample + AsSampleFormat {
    Connection::new(app_name, "source", rate, pa_stream_direction::PA_STREAM_RECORD)
        .map(|c| Source { conn: c })
}


#[derive(Debug)]
pub struct PulseError(pa_error_code);

impl fmt::Display for PulseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        unsafe {
            let errstr = ffi::CStr::from_ptr(pa_strerror(self.0 as i32));
            write!(f, "Pulse error: {}", errstr.to_str().unwrap())
        }
    }
}

impl error::Error for PulseError {
    fn description(&self) -> &str {
        unsafe {
            let errstr = ffi::CStr::from_ptr(pa_strerror(self.0 as i32));
            errstr.to_str().unwrap()
        }
    }
    fn cause(&self) -> Option<&error::Error> {
        None
    }
}
