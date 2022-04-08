use super::buffer::Buffer;
use super::event_loop::EventLoop;
use log::{debug, warn};
use nix::fcntl::{open, OFlag};
use nix::sys::epoll::EpollFlags;
use nix::sys::sendfile::sendfile;
use nix::sys::socket::shutdown;
use nix::sys::socket::{accept4, connect, getpeername, getsockname, setsockopt, socket, sockopt};
use nix::sys::socket::{AddressFamily, InetAddr, Shutdown};
use nix::sys::socket::{SockAddr, SockFlag, SockProtocol, SockType};
use nix::sys::stat::fstat;
use nix::sys::stat::Mode;
use nix::unistd::write;
use std::net::{SocketAddr, TcpListener};
use std::os::unix::prelude::AsRawFd;
use std::str::FromStr;
use std::sync::{Arc, Mutex};

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

pub trait EventSet {
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
    input_buf: Buffer,
    output_buf: Buffer,
    local_addr: String,
    peer_addr: String,
}

impl Connection {
    pub fn new(fd: i32) -> Self {
        assert!(fd > 0);
        let local_addr = format!("{}", getsockname(fd).unwrap());
        let peer_addr = format!("{}", getpeername(fd).unwrap());
        Connection {
            fd,
            state: State::Ready,
            input_buf: Buffer::new(),
            output_buf: Buffer::new(),
            local_addr,
            peer_addr,
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
        // TODO: add a exception handle
        match connect(sockfd, &sock_addr) {
            Ok(()) => debug!("a new connection: {}", sockfd),
            Err(e) => warn!("connect failed: {}", e),
        }
        return Connection::new(sockfd);
    }
    pub fn accept(listen_fd: i32) -> Self {
        let fd = accept4(listen_fd, SockFlag::SOCK_CLOEXEC | SockFlag::SOCK_NONBLOCK).unwrap();
        setsockopt(fd, sockopt::TcpNoDelay, &true).unwrap();
        setsockopt(fd, sockopt::KeepAlive, &true).unwrap();
        Connection::new(fd)
    }
    pub fn set_no_delay(&mut self, on: bool) {
        setsockopt(self.fd, sockopt::KeepAlive, &on).unwrap();
    }
    pub fn connected(&self) -> bool {
        self.state != State::Closed
    }
    pub fn get_peer_addr(&self) -> String {
        self.peer_addr.clone()
    }
    pub fn get_local_addr(&self) -> String {
        self.local_addr.clone()
    }
    pub fn dispatch(&mut self, revents: EpollFlags) -> State {
        self.state = State::Ready;
        if revents.is_readable() {
            self.input_buf.read(self.fd);
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
        return self.state;
    }
    pub fn get_fd(&self) -> i32 {
        self.fd
    }
    pub fn get_state(&self) -> State {
        self.state
    }
    // pub fn get_msg(&mut self) -> Vec<u8> {
    //     let buf = self.read_buf.to_owned();
    //     self.read_buf.clear();
    //     buf
    // }
    pub fn register_read(&mut self, event_loop: &mut EventLoop) {
        // self.read_buf.clear();
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
        self.shutdown();
    }
    pub fn shutdown(&self) {
        match shutdown(self.fd, Shutdown::Both) {
            Ok(()) => (),
            Err(e) => warn!("Shutdown {} occur {} error", self.fd, e),
        }
    }
    // TODO: 限速发送，定时发送一部分
    pub fn send_file(&mut self, file: &str) -> Option<usize> {
        let fd = open(file, OFlag::O_RDWR, Mode::S_IRUSR).unwrap();
        let stat = fstat(fd).unwrap();
        let size = sendfile(self.fd, fd, None, stat.st_size as usize).unwrap();
        Some(size)
    }
    pub fn send(&mut self, buf: &[u8]) {
        match write(self.fd, buf) {
            Ok(_) => (),
            Err(e) => warn!("send data error: {}", e),
        };
    }
    pub fn write_buf(&mut self, buf: &[u8]) {

        // TODO:
    }
    pub fn write(&mut self) {
        // TODO:
        // write(self.fd, &data).unwrap();
    }
    pub fn read_buf(&mut self) -> Vec<u8> {
        self.input_buf.read(self.fd);
        self.input_buf.read_buf()
    }
    pub fn read_msg(&mut self) -> Option<Vec<u8>> {
        match self.input_buf.read(self.fd) {
            Some(0) | None => None,
            Some(_) => self.input_buf.get_crlf_line(),
        }
        // let mut buf = [0u8; 4 * 1024];
        // while self.state != State::Finished && self.state != State::Closed {
        //     match read(self.fd, &mut buf) {
        //         Ok(0) => self.state = State::Finished,
        //         Ok(n) => {
        //             self.read_buf.extend_from_slice(&buf[0..n]);
        //             self.state = State::Reading;
        //             if n != buf.len() {
        //                 self.state = State::Finished;
        //                 debug!("Read data len: {}", n);
        //                 break;
        //             }
        //         }
        //         Err(Errno::EINTR) => debug!("Read EINTR error"),
        //         Err(Errno::EAGAIN) => debug!("Read EAGIN error"),
        //         Err(e) => {
        //             self.state = State::Closed;
        //             warn!("Read error: {}", e);
        //         }
        //     }
        //     // TODO: buffer replace vec
        //     if self.write_buf.len() >= 64 * 1024 {
        //         self.state = State::Reading;
        //         debug!("Send data size exceed 64kB");
        //         break;
        //     }
        // }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nix::sys::socket::socketpair;
    use std::cell::RefCell;
    use std::rc::Rc;
    #[test]
    fn test_send_rev_msg() {
        let (rev, send) = socketpair(
            AddressFamily::Inet,
            SockType::Stream,
            SockProtocol::Tcp,
            SockFlag::SOCK_CLOEXEC,
        )
        .unwrap();
        let rev = Rc::new(RefCell::new(Connection::new(rev)));
        let send = Rc::new(RefCell::new(Connection::new(rev)));
        assert_eq!(*rev.borrow_mut().connected(), true);
        assert_eq!(*send.borrow_mut().connected(), true);

        // *send.borrow_mut().send("");
    }
    #[test]
    fn test_send_rev_file() {}
}
