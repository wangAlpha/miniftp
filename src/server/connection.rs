use log::{debug, info, warn};
use nix::sys::epoll::EpollFlags;
use nix::sys::socket::getsockname;
use nix::sys::socket::shutdown;
use nix::sys::socket::Shutdown;
use nix::sys::socket::{accept4, setsockopt, sockopt};
use nix::sys::socket::{SockAddr, SockFlag};
use nix::unistd::{read, write};
use std::net::TcpListener;
use std::os::unix::prelude::AsRawFd;
use std::sync::{Arc, Mutex};

use super::server::EventLoop;
pub type ConnRef = Arc<Mutex<Connection>>;

#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub enum State {
    Reading,
    Ready,
    Writing,
    Finished,
    Closed,
}
const READABLE: u8 = 0b0001;
const WRITABLE: u8 = 0b0010;

trait EventSet {
    fn is_readable(&self) -> bool;
    fn is_writeable(&self) -> bool;
    fn is_close(&self) -> bool;
    fn is_error(&self) -> bool;
    fn is_hup(&self) -> bool;
}
impl EventSet for EpollFlags {
    fn is_readable(&self) -> bool {
        (*self & (EpollFlags::EPOLLIN | EpollFlags::EPOLLPRI)).bits() > 0
    }
    fn is_writeable(&self) -> bool {
        (*self & EpollFlags::EPOLLOUT).bits() > 0
    }
    fn is_close(&self) -> bool {
        (*self & EpollFlags::EPOLLHUP).bits() > 0 && !((*self & EpollFlags::EPOLLIN).bits() > 0)
    }
    fn is_error(&self) -> bool {
        (*self & EpollFlags::EPOLLERR).bits() > 0
    }
    fn is_hup(&self) -> bool {
        (*self & EpollFlags::EPOLLHUP).bits() > 0
    }
}
#[derive(Debug, Clone)]
pub struct Connection {
    fd: i32,
    state: State,
    write_buf: Vec<u8>,
    read_buf: Vec<u8>,
    event_loop: Arc<Option<EventLoop>>,
    event_addd: bool,
}

impl Connection {
    pub fn new(fd: i32) -> Self {
        Connection {
            fd,
            state: State::Ready,
            write_buf: Vec::new(),
            read_buf: Vec::new(),
            event_loop: Arc::new(None),
            event_addd: false,
        }
    }
    pub fn bind(addr: &str) -> (i32, TcpListener) {
        let listener = TcpListener::bind(addr).unwrap();
        (listener.as_raw_fd(), listener)
    }
    pub fn accept(listen_fd: i32, event_loop: &mut EventLoop) -> (i32, Self) {
        let fd = accept4(listen_fd, SockFlag::SOCK_CLOEXEC | SockFlag::SOCK_NONBLOCK).unwrap();
        setsockopt(fd, sockopt::TcpNoDelay, &true).unwrap();
        (fd, Connection::new(fd))
    }
    pub fn dispatch(&mut self, event_loop: &mut EventLoop, revents: EpollFlags) {
        debug!("connection {} state: {:?}", self.fd, self.state);
        if revents.is_readable() {
            self.read();
        }
        if revents.is_writeable() {
            self.write();
        }
        if revents.is_error() {
            self.state = State::Closed;
        }
        if revents.is_close() {
            self.state = State::Closed;
        }
    }
    pub fn get_fd(&self) -> i32 {
        self.fd
    }
    pub fn get_state(&self) -> State {
        self.state
    }
    pub fn get_msg(&mut self) -> Vec<u8> {
        self.read_buf.to_owned()
    }
    pub fn register_read(&mut self, event_loop: &mut EventLoop) {
        self.read_buf.clear();
        event_loop.reregister(
            self.fd,
            EpollFlags::EPOLLHUP
                | EpollFlags::EPOLLERR
                | EpollFlags::EPOLLIN
                | EpollFlags::EPOLLOUT
                | EpollFlags::EPOLLET,
        );
    }
    pub fn deregister(&self, event_loop: &mut EventLoop) {
        event_loop.deregister(self.fd);
        shutdown(self.fd, Shutdown::Both).unwrap();
    }
    pub fn send(&mut self, buf: &[u8]) {
        write(self.fd, buf).unwrap();
    }
    pub fn write_buf(&mut self, buf: &[u8]) {

        // TODO:
    }
    pub fn write(&mut self) {
        // TODO:
        // write(self.fd, &data).unwrap();
    }
    pub fn get_peer_address(&self) -> SockAddr {
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
