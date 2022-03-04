use nix::sys::epoll::{EpollCreateFlags, EpollEvent, EpollFlags};
use nix::sys::socket::accept4;
use nix::sys::socket::setsockopt;
use nix::sys::socket::sockopt;
use nix::sys::socket::SockFlag;
use nix::unistd::{close, read, write};
use std::sync::{Arc, Mutex};

use super::server::EventLoop;
pub type ConnRef = Arc<Mutex<Connection>>;
#[derive(Debug, Eq, PartialEq, Clone)]
enum State {
    Reading,
    Ready,
    Writing,
    Finished,
    Closed,
}

pub struct Connection {
    fd: i32,
    state: State,
    write_buf: Vec<u8>,
    read_buf: Vec<u8>,
    event_addd: bool,
}
impl Connection {
    pub fn new(fd: i32) -> Self {
        Connection {
            fd,
            state: State::Closed,
            write_buf: Vec::new(),
            read_buf: Vec::new(),
            event_addd: false,
        }
    }
    pub fn accept(listen_fd: i32) -> (i32, Self) {
        let fd = accept4(listen_fd, SockFlag::SOCK_CLOEXEC | SockFlag::SOCK_NONBLOCK).unwrap();
        setsockopt(fd, sockopt::TcpNoDelay, &true).unwrap();
        (fd, Connection::new(fd))
    }

    pub fn register_read(&self, event_loop: &mut EventLoop) {
        event_loop.reregister(self.fd, EpollFlags::EPOLLIN | EpollFlags::EPOLLPRI);
    }
    pub fn deregister(&self, event_loop: &mut EventLoop) {
        event_loop.deregister(self.fd);
        close(self.fd);
    }
    pub fn read(&self) {
        let mut buf = [0u8; 1024];
        match read(self.fd, &mut buf) {
            Ok(n) => print!("read len: {} data: {}", n, String::from_utf8_lossy(&buf)),
            Err(e) => println!("read error: {}", e),
        }
        println!("")
    }
}
