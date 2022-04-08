use log::{debug, error};
use nix::sys::uio::{readv, IoVec};
use std::{fmt, ptr};

/// A buffer class modeled after org.jboss.netty.buffer.ChannelBuffer
///
/// @code
/// +-------------------+------------------+------------------+
/// | prependable bytes |  readable bytes  |  writable bytes  |
/// |                   |     (CONTENT)    |                  |
/// +-------------------+------------------+------------------+
/// |                   |                  |                  |
/// 0      <=      readerIndex   <=   writerIndex    <=     size
/// @endcode

const DEFAULT_INIT_SIZE: usize = 1024;
#[derive(Clone)]
pub struct Buffer {
    data: Vec<u8>,
    read_index: usize,
    write_index: usize,
}

impl Buffer {
    pub fn new() -> Buffer {
        Buffer {
            data: vec![0u8; DEFAULT_INIT_SIZE],
            read_index: 0,
            write_index: 0,
        }
    }
    pub fn reset(&mut self) {
        self.read_index = 0;
        self.write_index = 0;
        self.data.resize(DEFAULT_INIT_SIZE, 0u8);
    }
    // Read data to buffer for file description
    pub fn read(&mut self, fd: i32) -> Option<usize> {
        let mut extrabuf = [0u8; 1024 * 64];
        let mut len = 0usize;
        let mut done = false;

        while !done {
            done = true;
            let writable = self.writable_bytes();
            let mut iov = [
                IoVec::from_mut_slice(&mut self.data[self.write_index..]),
                IoVec::from_mut_slice(&mut extrabuf),
            ];
            match readv(fd, &mut iov) {
                Ok(0) => {
                    error!("Read len: 0");
                }
                Ok(n) => {
                    if n <= writable {
                        self.write_index += n;
                    } else {
                        self.write_index = self.data.len();
                        self.append(&mut extrabuf[0..n - writable]);
                    }

                    if n == writable + extrabuf.len() {
                        done = false;
                        debug!("Read buffer again");
                    }
                    len += n;
                }
                Err(e) => {
                    error!("Read error: {}", e);
                }
            }
        }
        let s = String::from_utf8_lossy(&self.data[self.read_index..self.write_index]);
        debug!("Buffer read data len: {:?}{}", s, len);
        Some(len)
    }
    pub fn get_line(&mut self) -> Option<String> {
        if let Some(n) = self.find_eol() {
            let buf = &self.data[self.read_index..self.read_index + n + 1];
            self.read_index += buf.len();
            return Some(String::from_utf8(buf.to_vec()).unwrap());
        }
        None
    }
    pub fn get_crlf_line(&mut self) -> Option<Vec<u8>> {
        if let Some(n) = self.find_crlf() {
            let buf = &self.data[self.read_index..self.read_index + n + 2];
            self.read_index += buf.len();
            return Some(buf.to_vec());
        }
        None
    }
    pub fn read_buf(&mut self) -> Vec<u8> {
        let buf = self.bytes().to_vec();
        self.read_index = buf.len();
        buf
    }
    pub fn append(&mut self, buf: &[u8]) {
        if self.writable_bytes() < buf.len() {
            self.adjust_space(buf.len());
        }
        // self.data.extend_from_slice(buf);
        let count = buf.len();
        unsafe {
            let src = buf.as_ptr();
            let dst = self.data.as_mut_ptr().offset(self.write_index as isize);
            ptr::copy(src, dst, count);
        }
        self.write_index += count;
        println!(
            "write_index: {} buf len:{} data len: {}",
            self.write_index,
            buf.len(),
            self.data.len()
        );
    }
    fn adjust_space(&mut self, len: usize) {
        if self.remaining() < len {
            let size = self.write_index + len;
            let new_size = approximate_pow(size as u64) as usize;
            self.data.resize(new_size, 0);
            println!("new size: {} data: {}", new_size, self.data.len());
        } else {
            let readable = self.readable_bytes();
            self.left_shift();
            assert_eq!(readable, self.readable_bytes());
        }
    }
    // 可写区间大小
    fn writable_bytes(&self) -> usize {
        self.data.len() - self.write_index
    }
    // 可读区间大小
    fn readable_bytes(&mut self) -> usize {
        self.write_index - self.read_index
    }
    // 内部空间左移至开始
    fn left_shift(&mut self) {
        let new_size = self.write_index - self.read_index;
        unsafe {
            let src = self.data.as_mut_ptr().offset(self.read_index as isize);
            let dst = self.data.as_mut_ptr();
            ptr::copy(src, dst, new_size);
        }
        self.read_index = 0;
        self.write_index = new_size;
    }
    // 内部总共空闲的空间
    fn remaining(&mut self) -> usize {
        self.data.len() - self.readable_bytes()
    }
    // 可读区域
    fn bytes<'a>(&'a self) -> &'a [u8] {
        &self.data[self.read_index..self.write_index]
    }
    fn find_eol(&self) -> Option<usize> {
        self.bytes().iter().position(|&b| b == b'\n')
    }
    fn find_crlf(&self) -> Option<usize> {
        self.bytes().windows(2).position(|bytes| bytes == b"\r\n")
    }
}

fn approximate_pow(n: u64) -> u64 {
    let base = (n as f64).log2() + 1f64;
    2u64.pow(base as u32)
}
impl fmt::Debug for Buffer {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "Buffer[.. {}]", self.data.len())
    }
}

fn main() {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::os::unix::prelude::AsRawFd;
    #[test]
    fn test_buffer_append() {
        let mut buf = Buffer::new();
        assert_eq!(buf.readable_bytes(), 0);
        assert_eq!(buf.writable_bytes(), DEFAULT_INIT_SIZE);

        let s = [b'x'; 200];
        buf.append(&s);
        assert_eq!(buf.readable_bytes(), s.len());
        assert_eq!(buf.writable_bytes(), DEFAULT_INIT_SIZE - s.len());
        assert_eq!(buf.remaining(), DEFAULT_INIT_SIZE - s.len());

        let ss = "hello\r\n";
        buf.append(ss.as_bytes());
        let line = buf.get_crlf_line().unwrap();
        assert_eq!(line.len(), ss.len() + s.len());
        assert_eq!(buf.readable_bytes(), 0);
        assert_eq!(buf.writable_bytes(), DEFAULT_INIT_SIZE - line.len());

        let ss = "hello\r\n";
        buf.append(ss.as_bytes());
        let line = buf.get_line().unwrap();
        assert_eq!(line.len(), ss.len());
        assert_eq!(buf.readable_bytes(), 0);

        buf.reset();
        assert_eq!(buf.readable_bytes(), 0);
        assert_eq!(buf.writable_bytes(), DEFAULT_INIT_SIZE);
        assert_eq!(buf.get_line(), None);
        assert_eq!(buf.get_crlf_line(), None);
    }
    #[test]
    fn test_append_overflow() {
        let mut buf = Buffer::new();
        let bytes = "12345678123456781234567812345678".as_bytes();
        for _ in 0..33 {
            buf.append(bytes);
        }
        println!(
            "remaining: {} writable len: {}",
            buf.remaining(),
            buf.writable_bytes()
        );
        assert_eq!(bytes.len() * 33, buf.readable_bytes());
        assert_eq!(
            DEFAULT_INIT_SIZE * 2 - bytes.len() * 33,
            buf.writable_bytes()
        );
    }
    #[test]
    fn test_buffer_read() {
        let file = File::open("miniftp").unwrap();
        let metadata = file.metadata().unwrap();

        let mut buf = Buffer::new();
        let size = buf.read(file.as_raw_fd()).unwrap();
        assert_eq!(size, metadata.len() as usize);
        assert_eq!(buf.readable_bytes(), metadata.len() as usize);
    }
}
