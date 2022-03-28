use std::fmt::{self, Display, Formatter};
use std::io;
use std::result;
use std::str::Utf8Error;
use std::string::FromUtf8Error;

#[derive(Debug)]
pub enum Error {
    FromUtf8(FromUtf8Error),
    Io(io::Error),
    Msg(String),
    Utf8(Utf8Error),
}

impl Error {
    pub fn to_io_error(self) -> io::Error {
        match self {
            Error::Io(err) => err,
            Error::FromUtf8(_) | Error::Msg(_) | Error::Utf8(_) => io::ErrorKind::Other.into(),
        }
    }
}

impl Display for Error {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        match *self {
            Error::FromUtf8(ref err) => err.fmt(formatter),
            Error::Io(ref err) => err.fmt(formatter),
            Error::Utf8(ref err) => err.fmt(formatter),
            Error::Msg(ref msg) => write!(formatter, "{}", msg),
        }
    }
}

pub type Result<T> = result::Result<T, Error>;

impl<'a> From<&'a str> for Error {
    fn from(message: &'a str) -> Self {
        Error::Msg(message.to_string())
    }
}
