use super::poller::Poller;
use nix::sys::epoll::{EpollEvent, EpollFlags, EpollOp};
use std::collections::{HashMap, HashSet};
use std::net::TcpListener;
use std::os::unix::prelude::AsRawFd;
use std::sync::{Arc, Mutex};

pub const EVENT_LEVEL: EpollFlags = EpollFlags::EPOLLET;
pub const EVENT_READ: EpollFlags = EpollFlags::EPOLLIN;
pub const EVENT_ERR: EpollFlags = EpollFlags::EPOLLERR;
pub const EVENT_HUP: EpollFlags = EpollFlags::EPOLLHUP;

#[derive(Debug, Clone, Copy)]
pub enum Token {
    Listen(i32),
    Read(i32),
}

pub trait Handler: Sized {
    type Timeout;
    type Message;
    fn ready(&mut self, event_loop: &mut EventLoop, token: Token, revent: EpollFlags);
    fn notify(&mut self, event_loop: &mut EventLoop, token: Token, revent: EpollFlags);
}

#[derive(Debug, Clone)]
pub struct EventLoop {
    listener: Arc<TcpListener>,
    data_listener: Arc<Mutex<HashMap<i32, TcpListener>>>,
    data_conn: Arc<Mutex<HashSet<i32>>>,
    poller: Poller,
    run: bool,
}

impl EventLoop {
    pub fn new(listener: TcpListener) -> Self {
        let mut poller = Poller::new();
        let interest = EpollFlags::EPOLLHUP
            | EpollFlags::EPOLLERR
            | EpollFlags::EPOLLIN
            | EpollFlags::EPOLLOUT
            | EpollFlags::EPOLLET;
        poller.register(listener.as_raw_fd(), interest);
        EventLoop {
            listener: Arc::new(listener),
            data_listener: Arc::new(Mutex::new(HashMap::new())),
            run: true,
            poller,
            data_conn: Arc::new(Mutex::new(HashSet::new())),
        }
    }
    pub fn register(&mut self, listener: TcpListener, interest: EpollFlags) {
        let fd = listener.as_raw_fd();
        self.data_listener.lock().unwrap().insert(fd, listener);
        self.poller.register(fd, interest);
    }
    pub fn is_data_conn(&self, fd: i32) -> bool {
        self.data_listener.lock().unwrap().contains_key(&fd)
    }
    pub fn register_listen(&mut self, listener: TcpListener) {
        self.register(
            listener,
            EpollFlags::EPOLLHUP
                | EpollFlags::EPOLLERR
                | EpollFlags::EPOLLIN
                | EpollFlags::EPOLLOUT
                | EpollFlags::EPOLLET,
        );
    }
    pub fn reregister(&self, fd: i32, interest: EpollFlags) {
        let event = EpollEvent::new(interest, fd as u64);
        self.poller
            .update(EpollOp::EpollCtlAdd, fd, &mut Some(event));
    }
    pub fn deregister(&self, fd: i32) {
        self.poller.update(EpollOp::EpollCtlDel, fd, &mut None);
    }
    pub fn is_new_conn(&self, fd: i32) -> bool {
        self.listener.as_raw_fd() == fd || self.data_listener.lock().unwrap().contains_key(&fd)
    }
    pub fn run<H>(&mut self, handler: &mut H)
    where
        H: Handler,
    {
        while self.run {
            let cnt = self.poller.poll();
            // io ready event: connection
            for i in 0..cnt {
                let (fd, event) = self.poller.event(i);
                let token = if fd == self.listener.as_raw_fd() {
                    Token::Listen(fd)
                } else if self.data_listener.lock().unwrap().contains_key(&fd) {
                    Token::Listen(fd)
                } else {
                    Token::Read(fd)
                };
                handler.ready(self, token, event.events());
            }
            // io read and write event
            // for i in 0..cnt {
            //     let (fd, event) = self.poller.event(i);
            //     handler.notify(self, token, event.events());
            // }
        }
    }
}
