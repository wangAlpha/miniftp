use super::connection::{ConnRef, Connection};
use super::poller::Poller;
use nix::libc::*;
use nix::sys::epoll::{EpollEvent, EpollFlags, EpollOp};
use std::collections::HashMap;
use std::net::TcpListener;
use std::os::unix::prelude::AsRawFd;
use std::sync::{Arc, Mutex};

const EVENT_READ: EpollFlags = EpollFlags::EPOLLIN;
const EVENT_ERR: EpollFlags = EpollFlags::EPOLLERR;
const EVENT_HUP: EpollFlags = EpollFlags::EPOLLHUP;
pub trait Handler: Sized {
    type Timeout;
    type Message;
    fn ready(&mut self, event_loop: &mut EventLoop, fd: i32, revent: EpollEvent);
    fn notify(&mut self, event_loop: &mut EventLoop, msg: Self::Message);
}

#[derive(Debug)]
pub struct EventLoop {
    poller: Poller,
    run: bool,
}

impl EventLoop {
    pub fn new() -> Self {
        let poller = Poller::new();
        EventLoop { run: true, poller }
    }
    pub fn register(&mut self, fd: i32, interest: EpollFlags) {
        self.poller.register(fd, interest);
    }
    pub fn reregister(&self, fd: i32, interest: EpollFlags) {
        let event = EpollEvent::new(interest, fd as u64);
        self.poller
            .update(EpollOp::EpollCtlAdd, fd, &mut Some(event));
    }
    pub fn deregister(&self, fd: i32) {
        self.poller.update(EpollOp::EpollCtlDel, fd, &mut None);
    }
    pub fn run<H>(&mut self, handler: &mut H)
    where
        H: Handler,
    {
        while self.run {
            let cnt = self.poller.poll();
            // self.io_event(handler, cnt);
            for i in 0..cnt {
                let (fd, revent) = self.poller.event(i);
                handler.ready(self, fd, revent);
            }
            for i in 0..cnt {}
            // self.notify(handler, cnt);
        }
    }
}

pub struct FtpServer {
    conn_list: HashMap<i32, ConnRef>,
    listener: TcpListener,
    poller_fd: i32,
}

impl FtpServer {
    pub fn new(listener: TcpListener) -> Self {
        FtpServer {
            conn_list: HashMap::new(),
            listener,
            poller_fd: 0,
        }
    }
}

impl Handler for FtpServer {
    type Message = String;
    type Timeout = i32;
    fn ready(&mut self, event_loop: &mut EventLoop, fd: i32, events: EpollEvent) {
        let listen_fd = self.listener.as_raw_fd();
        let revents = events.events();
        if fd == listen_fd && (revents & EVENT_READ).bits() > 0 {
            let (fd, conn) = Connection::accept(listen_fd);
            self.conn_list.insert(fd, Arc::new(Mutex::new(conn)));
            self.conn_list[&fd]
                .lock()
                .unwrap()
                .register_read(event_loop);
            println!("register a new connection: {}", fd);
        } else if (revents & EVENT_READ).bits() > 0 {
            self.conn_list[&fd].lock().unwrap().read();
            // self.conn_list[fd as usize].
            // self.conn_list[token].r
            // read data from this connection
        } else if (revents & (EVENT_ERR | EVENT_HUP)).bits() > 0 {
            // event_loop.update(fd, EpollOp::EpollCtlDel, None);
            // close this connection
            println!("connection: {} occur error: {:?}", fd, revents);
            self.conn_list[&fd].lock().unwrap().deregister(event_loop);
            // TODO: connection dispatch
            self.conn_list.remove(&fd);
            println!("disconnection fd: {}", fd);
        }
    }
    fn notify(&mut self, event_loop: &mut EventLoop, msg: Self::Message) {}
}

pub fn run_server() {
    let listener = TcpListener::bind("0.0.0.0:8089").unwrap();
    let listen_fd = listener.as_raw_fd();
    println!("Tcp listener: {:?}", listener);

    let mut event_loop = EventLoop::new();
    event_loop.register(listen_fd, EpollFlags::EPOLLIN | EpollFlags::EPOLLERR);

    let mut ftpserver = FtpServer::new(listener);
    event_loop.run(&mut ftpserver);
}
