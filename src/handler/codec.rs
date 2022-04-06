use crate::handler::cmd::{Answer, Command};
use crate::handler::error::Error;
use std::io::{self, Write};

#[derive(Debug, Clone, Copy)]
pub struct FtpCodec;

#[derive(Debug, Clone, Copy)]
pub struct BytesCodec;

pub trait Decoder {
    type Item;
    type Error: From<io::Error>;
    fn decode(&mut self, src: &mut Vec<u8>) -> Result<Option<Self::Item>, Self::Error>;
    fn decode_eof(&mut self, buf: &mut Vec<u8>) -> Result<Option<Self::Item>, Self::Error> {
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
    fn encode(&mut self, item: Self::Item, dst: &mut Vec<u8>) -> Result<(), Self::Error>;
}

impl Decoder for FtpCodec {
    type Item = Command;
    type Error = io::Error;
    fn decode(&mut self, buf: &mut Vec<u8>) -> Result<Option<Self::Item>, Self::Error> {
        if let Some(index) = find_crlf(buf) {
            let (_, line) = buf.split_at(index); // Remove \r\n
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
    fn encode(&mut self, answear: Answer, buf: &mut Vec<u8>) -> io::Result<()> {
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
    type Item = Answer;
    type Error = io::Error;

    fn decode(&mut self, buf: &mut Vec<u8>) -> Result<Option<Self::Item>, Self::Error> {
        if buf.len() == 0 {
            return Ok(None);
        }
        if let Some(index) = find_crlf(buf) {
            let (_, line) = buf.split_at(index);
            return Ok(Answer::from(
                &String::from_utf8(line.to_vec()).unwrap(),
            ));
        } else {
            Ok(None)
        }
    }
}

impl Encoder for BytesCodec {
    type Item = Vec<u8>;
    type Error = io::Error;
    fn encode(&mut self, item: Self::Item, buf: &mut Vec<u8>) -> io::Result<()> {
        buf.extend(item);
        buf.extend(b"\r\n");
        Ok(())
    }
}

pub fn find_crlf(buf: &mut Vec<u8>) -> Option<usize> {
    buf.windows(2).position(|bytes| bytes == b"\r\n")
}

#[cfg(test)]
mod tests {
    use crate::handler::cmd::ResultCode;

    use super::*;
    #[test]
    fn test_encoder() {
        let mut codec = FtpCodec;
        let mut message = "bad sequence of commands";
        let answer = Answer::new(ResultCode::BadCmdSeq, message);

        let mut out = Vec::new();
        let result = "503 bad sequence of commands\r\n".as_bytes().to_vec();

        let code = answer.code;
        codec.encode(answer, &mut out).unwrap();
        assert_eq!(code, ResultCode::BadCmdSeq);
        assert_eq!(out, result);
    }
    #[test]
    fn test_decoder() {
        let mut ftp_codec = FtpCodec;
        let mut client_codec = BytesCodec;
        let mut message = "bad sequence of commands";
        let answer = Answer::new(ResultCode::BadCmdSeq, message);

        // Encode msg in server
        let mut msg = Vec::new();
        let code = answer.code;
        ftp_codec.encode(answer, &mut msg).unwrap();

        // Decode msg in client
        let result = client_codec.decode(&mut msg).unwrap().unwrap();
        let new_msg = result.message;
        assert_eq!(result.code, code);
        assert_eq!(message, new_msg);
    }
}
