use log::{debug, info, warn};
use nix::sys::epoll::EpollFlags;
use nix::sys::socket::{accept4, connect, setsockopt, sockopt};
use nix::sys::socket::{getpeername, shutdown, socket, Shutdown};
use nix::sys::socket::{AddressFamily, InetAddr, SockAddr, SockFlag, SockProtocol, SockType};
use nix::unistd::{read, write};
use std::net::{SocketAddr, TcpListener};
use std::os::unix::prelude::AsRawFd;
use std::str::FromStr;
use std::sync::{Arc, Mutex};

use super::server::{ConTyp, EventLoop, Token};
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
}

impl Connection {
    pub fn new(fd: i32) -> Self {
        Connection {
            fd,
            state: State::Ready,
            write_buf: Vec::new(),
            read_buf: Vec::new(),
        }
    }
    pub fn bind(addr: &str) -> (i32, TcpListener) {
        let listener = TcpListener::bind(addr).unwrap();
        (listener.as_raw_fd(), listener)
    }
    pub fn connect(addr: &str) -> Connection {
        let sockfd = socket(
            AddressFamily::Inet,
            SockType::Stream,
            SockFlag::SOCK_CLOEXEC,
            SockProtocol::Tcp,
        )
        .unwrap();

        let addr = SocketAddr::from_str(addr).unwrap();
        let inet_addr = InetAddr::from_std(&addr);
        let sock_addr = SockAddr::new_inet(inet_addr);

        match connect(sockfd, &sock_addr) {
            Ok(()) => println!("Connection success!"),
            Err(e) => println!("Connection failed: {}", e),
        }
        return Connection::new(sockfd);
    }
    pub fn accept(listen_fd: i32) -> Self {
        let fd = accept4(listen_fd, SockFlag::SOCK_CLOEXEC | SockFlag::SOCK_NONBLOCK).unwrap();
        setsockopt(fd, sockopt::TcpNoDelay, &true).unwrap();
        let mut c = Connection::new(fd);
        c
    }

    pub fn dispatch(&mut self, revents: EpollFlags) -> State {
        debug!("connection {} state: {:?}", self.fd, self.state);
        if revents.is_readable() {
            self.read();
        }
        if revents.is_writeable() {
            self.write();
        }
        if revents.is_error() {
            // self.state = State::Closed;
        }
        if revents.is_close() {
            self.state = State::Closed;
        }
        return self.state;
    }
    pub fn get_fd(&self) -> i32 {
        self.fd
    }
    pub fn get_state(&self) -> State {
        self.state
    }
    pub fn get_msg(&mut self) -> Vec<u8> {
        self.state = State::Reading;
        let buf = self.read_buf.to_owned();
        self.read_buf.clear();
        buf
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
    pub fn shutdown(&self) {
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
        let addr = getpeername(self.fd).expect("get peer socket address failed");
        addr
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
