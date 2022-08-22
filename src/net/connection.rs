use super::buffer::Buffer;
use super::event_loop::EventLoop;
use super::event_loop::*;
use super::socket::Socket;
use log::warn;
use nix::fcntl::{fcntl, open, FcntlArg, OFlag};
use nix::sys::epoll::EpollFlags;
use nix::sys::sendfile::sendfile;
use nix::sys::socket::Shutdown;
use nix::sys::socket::{getpeername, getsockname, shutdown};
use nix::sys::stat::Mode;
use nix::unistd::{close, write};
use std::os::unix::prelude::AsRawFd;
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
    sock: Socket,
    state: State,
    input_buf: Buffer,
    output_buf: Buffer,
    local_addr: String,
    peer_addr: String,
    revents: EpollFlags,
}

impl Connection {
    pub fn new(sock: Socket) -> Self {
        assert!(sock.as_raw_fd() > 0);
        let local_addr = format!("{}", getsockname(sock.as_raw_fd()).unwrap());
        let peer_addr = format!("{}", getpeername(sock.as_raw_fd()).unwrap());
        Connection {
            sock,
            state: State::Ready,
            input_buf: Buffer::new(),
            output_buf: Buffer::new(),
            local_addr,
            peer_addr,
            revents: EpollFlags::empty(),
        }
    }
    pub fn set_revents(&mut self, revents: &EpollFlags) {
        self.revents = revents.clone();
    }
    pub fn get_revents(&self) -> EpollFlags {
        self.revents
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
            self.input_buf.read(self.sock.as_raw_fd());
        }
        if revents.is_writeable() {
            // self.write();
        }
        if revents.is_error() {
            self.state = State::Closed;
        }
        if revents.is_close() {
            self.state = State::Closed;
        }
        return self.state;
    }
    pub fn get_fd(&self) -> Socket {
        self.sock.clone()
    }
    pub fn get_state(&self) -> State {
        self.state
    }
    pub fn register_read(&mut self, event_loop: &mut EventLoop) {
        event_loop.reregister(
            self.sock.as_raw_fd(),
            EVENT_HUP | EVENT_ERR | EVENT_WRIT | EVENT_READ | EVENT_LEVEL,
        );
    }
    pub fn deregister(&mut self, event_loop: &mut EventLoop) {
        event_loop.deregister(self.sock.as_raw_fd());
        self.shutdown();
    }
    pub fn shutdown(&mut self) {
        self.state = State::Closed;
        match shutdown(self.sock.as_raw_fd(), Shutdown::Both) {
            Ok(()) => (),
            Err(e) => warn!("Shutdown {} occur {} error", self.sock.as_raw_fd(), e),
        }
    }
    // 限速发送，定时发送一部分
    pub fn send_file(
        &mut self,
        file: Option<&str>,
        fd: i32,
        off: Option<i64>,
        size: usize,
    ) -> Option<usize> {
        let mut off64 = off.unwrap_or(0);
        let off = if off.is_none() { None } else { Some(&mut off64) };
        if let Some(file) = file {
            let fd = open(file, OFlag::O_RDWR, Mode::S_IRUSR).unwrap();
            let size = sendfile(self.sock.as_raw_fd(), fd, off, size).unwrap();
            close(fd).expect("Couldn't close file");
            return Some(size);
        } else {
            let size = sendfile(self.sock.as_raw_fd(), fd, off, size).unwrap();
            return Some(size);
        }
    }
    pub fn send(&mut self, buf: &[u8]) {
        match write(self.sock.as_raw_fd(), buf) {
            Ok(_) => (),
            Err(e) => {
                warn!("Send data error: {}", e)
            }
        };
    }
    pub fn read_buf(&mut self) -> Vec<u8> {
        self.input_buf.read(self.sock.as_raw_fd());
        self.input_buf.read_buf()
    }
    pub fn read_msg(&mut self) -> Option<Vec<u8>> {
        match self.input_buf.read(self.sock.as_raw_fd()) {
            Some(0) | None => None,
            Some(_) => self.input_buf.get_crlf_line(),
        }
    }
}
impl Drop for Connection {
    fn drop(&mut self) {
        if 0 > fcntl(self.sock.as_raw_fd(), FcntlArg::F_GETFL).unwrap() {
            self.shutdown();
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use nix::sys::socket::*;
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
        let (rev, send) = (Socket(rev), Socket(send));
        let rev = Rc::new(RefCell::new(Connection::new(rev)));
        let send = Rc::new(RefCell::new(Connection::new(send)));
        assert_eq!((*rev.borrow_mut()).connected(), true);
        assert_eq!((*send.borrow_mut()).connected(), true);

        // *send.borrow_mut().send("");
    }
    #[test]
    fn test_send_rev_file() {}
}
