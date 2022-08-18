use log::{debug, warn};
use nix::sys::socket::{accept4, bind, connect, setsockopt, socket, sockopt};
use nix::sys::socket::{AddressFamily, InetAddr};
use nix::sys::socket::{SockAddr, SockFlag, SockProtocol, SockType};
use std::net::SocketAddr;
use std::os::unix::prelude::{AsRawFd, RawFd};
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub struct Socket(pub(crate) i32);

lazy_static! {
    static ref NONBLOCKING_CLOEXEC: SockFlag = SockFlag::SOCK_CLOEXEC | SockFlag::SOCK_NONBLOCK;
}

impl Socket {
    // create a nonblocking socket
    pub fn bind(addr: &str) -> Self {
        let sockfd = socket(
            AddressFamily::Inet,
            SockType::Stream,
            SockFlag::SOCK_CLOEXEC | SockFlag::SOCK_NONBLOCK,
            SockProtocol::Tcp,
        )
        .unwrap();
        let sock_addr = inet_addr(addr);
        bind(sockfd, &sock_addr).unwrap();
        Socket(sockfd)
    }

    pub fn set_no_delay(&mut self, on: bool) {
        setsockopt(self.0, sockopt::TcpNoDelay, &on).unwrap();
    }
    pub fn set_keep_alive(&mut self, on: bool) {
        setsockopt(self.0, sockopt::KeepAlive, &on).unwrap();
    }
    pub fn set_reuse_addr(&mut self, on: bool) {
        setsockopt(self.0, sockopt::ReuseAddr, &on).unwrap();
    }
    pub fn set_reuse_port(&mut self, port: u16) {
        setsockopt(self.0, sockopt::ReusePort, &true).unwrap();
    }
    pub fn accept(sockfd: i32) -> Self {
        let connfd = accept4(sockfd, *NONBLOCKING_CLOEXEC).unwrap();
        Socket(connfd)
    }
    pub fn connect(addr: &str) -> Self {
        let sockfd = socket(
            AddressFamily::Inet,
            SockType::Stream,
            SockFlag::SOCK_CLOEXEC,
            SockProtocol::Tcp,
        )
        .unwrap();

        let sock_addr = inet_addr(addr);
        // TODO: add a exception handle
        match connect(sockfd, &sock_addr) {
            Ok(()) => debug!("a new connection: {}", sockfd),
            Err(e) => warn!("connect failed: {}", e),
        }
        Socket(sockfd)
    }
}

pub fn inet_addr(addr: &str) -> SockAddr {
    let addr = SocketAddr::from_str(addr).unwrap();
    let inet_addr = InetAddr::from_std(&addr);
    SockAddr::new_inet(inet_addr)
}

impl AsRawFd for Socket {
    fn as_raw_fd(&self) -> RawFd {
        self.0
    }
}
