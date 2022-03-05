use log::{info, warn};
use nix::sys::epoll::{EpollCreateFlags, EpollEvent, EpollFlags};
use nix::sys::socket::shutdown;
use nix::sys::socket::Shutdown;
use nix::sys::socket::{accept4, setsockopt, sockopt};
use nix::sys::socket::{SockAddr, SockFlag};
use nix::sys::{ptrace::Event, socket::getsockname};
use nix::unistd::{read, write};
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
            state: State::Ready,
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
    pub fn fd(&self) -> i32 {
        self.fd
    }
    pub fn register_read(&mut self, event_loop: &mut EventLoop) {
        self.read_buf.clear();
        event_loop.reregister(self.fd, EpollFlags::EPOLLIN | EpollFlags::EPOLLPRI);
    }
    pub fn deregister(&self, event_loop: &mut EventLoop) {
        event_loop.deregister(self.fd);
        shutdown(self.fd, Shutdown::Both).unwrap();
    }
    pub fn dispatch(&mut self, event_loop: &mut EventLoop, revents: EpollFlags) {
        if self.state == State::Closed {
            self.deregister(event_loop);
        }
    }
    pub fn send(&mut self, data: &[u8]) {
        write(self.fd, &data).unwrap();
    }
    pub fn peer_address(&self) -> SockAddr {
        getsockname(self.fd).expect("get peer socket address failed")
    }
    pub fn read(&mut self) {
        let mut buf = [0u8; 1024];
        match read(self.fd, &mut buf) {
            Ok(0) => self.state = State::Closed,
            Ok(n) => {
                self.read_buf.extend_from_slice(&buf[0..n]);
                self.state = if n == buf.len() {
                    State::Reading
                } else {
                    State::Finished
                };
                print!("read len: {} data: {}", n, String::from_utf8_lossy(&buf));
            }
            Err(e) => {
                self.state = State::Closed;
                warn!("read error: {}", e);
            }
        }
    }
}
