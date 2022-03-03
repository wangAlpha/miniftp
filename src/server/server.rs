use super::poller::Poller;
use nix::sys::epoll::{EpollEvent, EpollFlags, EpollOp};
use nix::sys::sendfile;
use nix::sys::socket::accept4;
use nix::sys::socket::setsockopt;
use nix::sys::socket::sockopt;
use nix::sys::socket::SockFlag;
use nix::unistd::{close, read, write};
use std::net::TcpListener;
use std::os::unix::prelude::AsRawFd;
pub struct Token(pub usize);
pub trait Handler: Sized {
    type Timeout;
    type Message;
    fn ready(&mut self, event_loop: &mut EventLoop<Self>, fd: EventSet) {}
    fn notify(&mut self, event_loop: &mut EventLoop<Self>, msg: Self::Message) {}
}

#[derive(Debug)]
pub struct EventLoop {
    poller: Poller,
    run: bool,
}

impl<H: Handler> EventLoop {
    pub fn new() -> Self {
        let poller = Poller::new();
        EventLoop { run: true, poller }
    }
    pub fn register(&self, listener: &TcpListener, interest: EpollFlags) {
        self.poller.register(listener, interest);
    }
    pub fn run(&self, handler: H) {
        while self.run {
            let events = self.poller.poll();
            self.io_process(&mut handler, events);
            self.notify(&mut handler);
            // self.poller.update(op, token, event)
        }
    }
    pub fn handle_event(&self, revents: u64) {
        if (revents & EpollFlags::EPOLLERR).bits() > 0 {}
        if (revents & EpollFlags::EPOLLHUP).bits() > 0 {}
        if (revents & EpollFlags::EPOLLIN).bits() > 0 {}
    }
    // readable event
    pub fn io_process(&self, handler: &mut H, cnt: usize) {
        for i in 0..cnt {
            let (fd, revent) = self.poller.event(i);
            handler.ready(&self, fd, revent);
        }
    }
    // readable event
    pub fn notify(&self, handler: &mut H) {}
}

pub struct FtpServer {
    conn_list: Vec<Connection>,
    listener: TcpListener,
}

impl FtpServer {
    pub fn new(listener: TcpListener) -> Self {
        FtpServer {
            conn_list: Vec::with_capacity(1024),
            listener,
        }
    }
}
impl Handler for FtpServer {
    type Message = String;
    fn ready(&mut self, event_loop: &mut EventLoop<FtpServer>, token: Token, events: EpollFlags::) {
        // if (self.revent & EVENT_ERROR).bits() > 0 {
        //     // errorCallback_();
        // }
        // if (self.revent & EVENT_READABLE).bits() > 0 {
        //     // if (readCallback_) readCallback_(receiveTime);
        // }
        // if (self.revent & EVENT_WRITEABLE).bits() > 0 {
        //     // if (writeCallback_) writeCallback_();
        // }
        if token > 0 {
            let listen_fd = self.listener.as_raw_fd();

            let fd = accept4(listen_fd, SockFlag::SOCK_CLOEXEC | SockFlag::SOCK_NONBLOCK).unwrap();
            setsockopt(fd, sockopt::TcpNoDelay, 1);
            self.conn_list.push(Arc::new(Connection::new(fd)));
        } else {
            // TODO: check to read and write state, either close thise connection.
            // close(fd)
        }
    }
    fn notify(&mut self, event_loop: &mut EventLoop<FtpServer>, msg: Self::Message) {}
}

pub fn run_server() {
    let listener = TcpListener::bind("0.0.0.0:8089").unwrap();
    println!("Tcp listener: {:?}", listener);
    let mut event_loop = EventLoop::new();
    event_loop
        .register(&listener, EpollFlags::EPOLLIN | EpollFlags::EPOLLERR)
        .unwrap();
    let mut ftpserver = FtpServer::new(listener);
    event_loop.run(&mut ftpserver).unwrap();
}
