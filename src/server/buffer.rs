use nix::sys::uio::{readv, writev};

// pub fn read_fd(fd: i32) -> usize {
//     let mut extrabuf: IoVec<u8> = [0u8; 65536];
//     let mut buf: IoVec<u8> = [0u8; 65536];

//     let mut iov: [IoVec<u8>] = [extrabuf, buf];
//     let len = readv(fd, iov).unwrap();
// }
