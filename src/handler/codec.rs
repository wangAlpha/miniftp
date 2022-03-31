use super::cmd::{Answer, Command};
use super::error::Error;
use bytes::BytesMut;
use std::io::{self, Write};

#[derive(Debug, Clone, Copy)]
pub struct BytesCodec;

#[derive(Debug, Clone, Copy)]
pub struct FtpCodec;

pub trait Decoder {
    type Item;
    type Error: From<io::Error>;
    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error>;
    fn decode_eof(&mut self, buf: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        match self.decode(buf)? {
            Some(frame) => Ok(Some(frame)),
            None => {
                if buf.is_empty() {
                    Ok(None)
                } else {
                    Err(io::Error::new(io::ErrorKind::Other, "bytes remaining on stream").into())
                }
            }
        }
    }
}
pub trait Encoder {
    type Item;
    type Error: From<io::Error>;
    fn encode(&mut self, item: Self::Item, dst: &mut BytesMut) -> Result<(), Self::Error>;
}

impl Decoder for FtpCodec {
    type Item = Command;
    type Error = io::Error;
    fn decode(&mut self, buf: &mut BytesMut) -> io::Result<Option<Command>> {
        if let Some(index) = find_crlf(buf) {
            let line = buf.split_to(index);
            buf.split_to(2); // Remove \r\n
            Command::new(line.to_vec())
                .map(|cmd| Some(cmd))
                .map_err(Error::to_io_error)
        } else {
            Ok(None)
        }
    }
}

impl Encoder for FtpCodec {
    type Item = Answer;
    type Error = io::Error;

    fn encode(&mut self, answear: Answer, buf: &mut BytesMut) -> io::Result<()> {
        let mut buffer = vec![];
        if answear.message.is_empty() {
            write!(buffer, "{}\r\n", answear.code as u32)?;
        } else {
            write!(buffer, "{} {}\r\n", answear.code as u32, answear.message)?
        }
        buf.extend(&buffer);
        Ok(())
    }
}

impl Decoder for BytesCodec {
    type Item = Vec<u8>;
    type Error = io::Error;

    fn decode(&mut self, buf: &mut BytesMut) -> io::Result<Option<Vec<u8>>> {
        if buf.len() == 0 {
            return Ok(None);
        }
        let data = buf.to_vec();
        buf.clear();
        Ok(Some(data))
    }
}
pub fn find_crlf(buf: &mut BytesMut) -> Option<usize> {
    buf.windows(2).position(|bytes| bytes == b"\r\n")
}

#[cfg(test)]
mod tests {
    use crate::handler::cmd::ResultCode;

    use super::*;
    use std::path::PathBuf;
    // #[test]
    // fn test_encoder() {
    //     let mut codec = FtpCodec;
    //     let mut message = "bad sequence of commands";
    //     let answer = Answer::new(ResultCode)
    // }
}
