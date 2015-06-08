use libc::{c_void, c_int, c_char, free};
use std::ptr;
use std::borrow::Cow;
use std::fmt;
use alsa;
use std::ffi::CStr;

const INVALID_STRING: c_int = 1;

#[derive(Debug)]
pub struct Error(Option<Cow<'static, str>>, c_int);

pub type Result<T> = ::std::result::Result<T, Error>;

#[inline]
pub fn check(f: &'static str, r: c_int) -> Result<c_int> {
    if r < 0 { Err(Error::new(Some(f.into()), r)) }
    else { Ok(r) }
}

pub fn from_const<'a>(func: &'static str, s: *const c_char) -> Result<&'a str> {
    if s == ptr::null() { return Err(Error::invalid_str(func)) };
    let cc = unsafe { CStr::from_ptr(s) };
    ::std::str::from_utf8(cc.to_bytes()).map_err(|_| Error::invalid_str(func))
}

pub fn from_alloc(func: &'static str, s: *mut c_char) -> Result<String> {
    if s == ptr::null_mut() { return Err(Error::invalid_str(func)) };
    let c = unsafe { CStr::from_ptr(s) };
    let ss = try!(::std::str::from_utf8(c.to_bytes()).map_err(|_| {
        unsafe { free(s as *mut c_void); }
        Error::invalid_str(func)
    })).to_string();
    unsafe { free(s as *mut c_void); }
    Ok(ss)
}

impl Error {
    pub fn new(func: Option<Cow<'static, str>>, res: c_int) -> Error { Error(func, res) }
    fn invalid_str(func: &'static str) -> Error { Error(Some(func.into()), INVALID_STRING) }
    pub fn code(&self) -> c_int { self.1 }
}

impl ::std::error::Error for Error {
    fn description(&self) -> &str { "ALSA error" } 
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let cc = unsafe { CStr::from_ptr(alsa::snd_strerror(self.1)) };
        let c = ::std::str::from_utf8(cc.to_bytes()).unwrap_or_else(|_| "(invalid UTF8)");
        match &self.0 {
            &Some(ref s) => write!(f, "ALSA error: '{}' (code {}) from function '{}'", c, self.1, s),
            &None => write!(f, "ALSA error: '{}' (code {})", c, self.1),
        }
    }
}

