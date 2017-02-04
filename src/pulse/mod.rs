use std::*;
use std::io::Read;
use sample;

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


fn sample_spec<F, S>(rate: u32) -> pa_sample_spec
    where F: sample::Frame<Sample=S>,
          S: sample::Sample + AsSampleFormat {
    assert!(F::n_channels() <= u8::MAX as usize);
    pa_sample_spec {
        format:   F::Sample::sample_format(),
        rate:     rate,
        channels: F::n_channels() as u8,
    }
}


pub struct Source<F: sample::Frame> {
    conn: *mut pa_simple,
    phantom: marker::PhantomData<F>,
}

impl<F> io::Read for Source<F>
    where F: sample::Frame {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut err_code = pa_error_code::PA_OK;
        unsafe {
            pa_simple_read(
                self.conn,
                buf.as_mut_ptr() as _,
                buf.len(),
                &mut err_code as *mut _ as *mut i32,
            );
        }
        if err_code != pa_error_code::PA_OK {
            return Err(io::Error::new(io::ErrorKind::Other, PulseError(err_code)));
        }
        Ok(buf.len())
    }
}

impl<F> iter::Iterator for Source<F>
    where F: sample::Frame {
    type Item = F;
    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            debug_assert_eq!(mem::size_of::<F>(), F::n_channels() * mem::size_of::<F::Sample>());
            let mut frame: F = mem::uninitialized();
            let buf = slice::from_raw_parts_mut(mem::transmute::<&F, *mut u8>(&mut frame), mem::size_of::<F>());
            self.read(buf).unwrap();
            Some(frame)
        }
    }
}

impl<F> Drop for Source<F>
    where F: sample::Frame {
    fn drop(&mut self) {
        unsafe { pa_simple_free(self.conn); }
    }
}

pub fn source<F, S>(app_name: &str, rate: u32) -> Result<Source<F>, Box<error::Error>>
    where F: sample::Frame<Sample=S>,
          S: sample::Sample + AsSampleFormat {
    let s = unsafe {
        let c_app_name = try!(ffi::CString::new(app_name));
        let c_stream_name = ffi::CString::new("source").unwrap();
        let mut err_code = pa_error_code::PA_OK;
        let s = pa_simple_new(
            ptr::null(),                           // Use the default server.
            c_app_name.as_ptr(),
            pa_stream_direction::PA_STREAM_RECORD,
            ptr::null(),                           // Use the default device.
            c_stream_name.as_ptr(),
            &sample_spec::<F, F::Sample>(rate),
            ptr::null(),                           // Use default channel map
            ptr::null(),                           // Use default buffering attributes.
            &mut err_code as *mut _ as *mut i32,
        );
        if err_code != pa_error_code::PA_OK {
            return Err(Box::from(PulseError(err_code)));
        }
        s
    };

    Ok(Source {
        conn: s,
        phantom: marker::PhantomData,
    })
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


pub trait AsSampleFormat {
    fn sample_format() -> pa_sample_format;
}

impl AsSampleFormat for u8 {
    fn sample_format() -> pa_sample_format { pa_sample_format::PA_SAMPLE_U8 }
}

#[cfg(target_endian = "little")]
mod endian {
    use super::*;
    impl AsSampleFormat for i16 {
        fn sample_format() -> pa_sample_format { pa_sample_format::PA_SAMPLE_S16LE }
    }
    impl AsSampleFormat for sample::I24 {
        fn sample_format() -> pa_sample_format {
            assert_eq!(4, mem::size_of::<Self>());
            pa_sample_format::PA_SAMPLE_S24_32LE
        }
    }
    impl AsSampleFormat for f32 {
        fn sample_format() -> pa_sample_format { pa_sample_format::PA_SAMPLE_FLOAT32LE }
    }
}

#[cfg(target_endian = "big")]
mod endian {
    use super::*;
    impl AsSampleFormat for i16 {
        fn sample_format() -> pa_sample_format { pa_sample_format::PA_SAMPLE_S16BE }
    }
    impl AsSampleFormat for sample::I24 {
        fn sample_format() -> pa_sample_format {
            assert_eq!(4, mem::size_of::<Self>());
            pa_sample_format::PA_SAMPLE_S24_32BE
        }
    }
    impl AsSampleFormat for f32 {
        fn sample_format() -> pa_sample_format { pa_sample_format::PA_SAMPLE_FLOAT32BE }
    }
}
