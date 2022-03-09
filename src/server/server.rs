use super::connection::{ConnRef, Connection, State};
use super::poller::Poller;
use super::queue::{BlockingQueue, BlockingQueueRef};
use crate::threadpool::threadpool::ThreadPool;
use log::{debug, info, warn};
use nix::sys::epoll::{EpollEvent, EpollFlags, EpollOp};
use nix::sys::signal::{SigHandler, Signal};
use nix::sys::socket::{sockopt::TcpCongestion, Shutdown};
use nix::sys::uio::{readv, writev};
use nix::sys::{signal::signal, socket::shutdown};
use num_cpus;
use std::hash::{Hash, Hasher};
use std::net::TcpListener;
use std::os::unix::prelude::AsRawFd;
use std::sync::Arc;
use std::sync::Mutex;

use std::{collections::HashMap, fmt::Debug};

pub const EVENT_LEVEL: EpollFlags = EpollFlags::EPOLLET;
pub const EVENT_READ: EpollFlags = EpollFlags::EPOLLIN;
pub const EVENT_ERR: EpollFlags = EpollFlags::EPOLLERR;
pub const EVENT_HUP: EpollFlags = EpollFlags::EPOLLHUP;
// pub struct Token(i32);
// const SERVER: Token = Token(0);
// type EventLoopRef = Arc<Mutex<EventLoop>>;
pub trait Handler: Sized {
    type Timeout;
    type Message;
    fn ready(&mut self, event_loop: &mut EventLoop, fd: i32, revent: EpollFlags);
    fn notify(&mut self, event_loop: &mut EventLoop, fd: i32, revent: EpollFlags);
}

pub fn signal_ignore() {
    let signal_set = [
        Signal::SIGPIPE,
        // Signal::SIGINT
    ];
    for sig in signal_set {
        unsafe {
            signal(sig, SigHandler::SigIgn).unwrap();
        }
    }
}

#[derive(Debug, Clone)]
pub struct EventLoop {
    listener: Arc<TcpListener>,
    data_listener: Arc<Mutex<HashMap<i32, TcpListener>>>,
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
        }
    }
    pub fn register(&mut self, listener: TcpListener, interest: EpollFlags) {
        let fd = listener.as_raw_fd();
        self.data_listener.lock().unwrap().insert(fd, listener);
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
    pub fn is_new_conn(&self, fd: i32) -> bool {
        self.listener.as_raw_fd() == fd || self.data_listener.lock().unwrap().contains_key(&fd)
    }
    pub fn is_data_conn(&self, fd: i32) -> bool {
        self.data_listener.lock().unwrap().contains_key(&fd)
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
                handler.ready(self, fd, event.events());
            }
            // io read and write event
            for i in 0..cnt {
                let (fd, event) = self.poller.event(i);
                handler.notify(self, fd, event.events());
            }
        }
    }
}
// pub enum Mode {}

type TaskQueueRef = BlockingQueueRef<(ConnRef, Vec<u8>)>;
pub struct FtpServer {
    conn_list: HashMap<i32, ConnRef>,
    request_queue: TaskQueueRef,
    worker_pool: ThreadPool,
}

impl FtpServer {
    pub fn new(event_loop: &mut EventLoop) -> Self {
        let q: TaskQueueRef = BlockingQueueRef::new(BlockingQueue::new(64));
        let pool = ThreadPool::new(num_cpus::get());
        for _ in 0..num_cpus::get() {
            let q_clone = q.clone();
            let mut event_loop_clone = event_loop.clone();
            pool.execute(move || loop {
                let (conn, msg) = q_clone.pop_front();
                let (fd, listener) = Connection::bind("0.0.0.0:8090");
                event_loop_clone.register(
                    listener,
                    EpollFlags::EPOLLHUP
                        | EpollFlags::EPOLLERR
                        | EpollFlags::EPOLLIN
                        | EpollFlags::EPOLLOUT
                        | EpollFlags::EPOLLET,
                );
                debug!("register a new listener: {:?}", fd);
            });
        }
        FtpServer {
            conn_list: HashMap::new(),
            request_queue: q,
            worker_pool: pool,
        }
    }
}

impl Handler for FtpServer {
    type Message = String;
    type Timeout = i32;
    fn ready(&mut self, event_loop: &mut EventLoop, token: i32, revents: EpollFlags) {
        debug!("a new event token: {}", token);
        if event_loop.is_new_conn(token) {
            // assert_ne!((revents & EVENT_READ).bits(), 0);
            let (fd, mut conn) = Connection::accept(token, event_loop);
            conn.register_read(event_loop);
            if !event_loop.is_data_conn(fd) {
                debug!("register a new cmd connection: {}", fd);
            } else {
                debug!("register a new data connection: {}", fd);
                event_loop.deregister(token);
            }
            debug!("connection address: {:?}", conn.get_peer_address());
            self.conn_list.insert(fd, Arc::new(Mutex::new(conn)));
        } else {
            self.conn_list[&token]
                .lock()
                .unwrap()
                .dispatch(event_loop, revents);
            let clone = self.conn_list[&token].clone();
            let msg = self.conn_list[&token].lock().unwrap().get_msg();
            self.request_queue.push_back((clone, msg));
            if self.conn_list[&token].lock().unwrap().get_state() == State::Closed {
                self.conn_list[&token]
                    .lock()
                    .unwrap()
                    .deregister(event_loop);
                self.conn_list.remove(&token);
                debug!("disconnection fd: {}", token);
            }
        }
    }
    fn notify(&mut self, event_loop: &mut EventLoop, fd: i32, revents: EpollFlags) {}
}
// impl Clone for EventLoop {
//     fn clone(&self) -> EventLoop {
//         EventLoop {
//             listener: self.listener.clone(),
//             data_listener: self.data_listener.clone(),
//             poller: self.poller.clone(),
//             run: self.run,
//         }
//     }

//     fn clone_from(&mut self, source: &Self) {
//         self.listener = source.listener.clone();
//         self.data_listener = source.data_listener.clone();
//         self.poller = source.poller.clone();
//         self.run = source.run;
//     }
// }

pub fn run_server() {
    let (_, listener) = Connection::bind("0.0.0.0:8089");
    debug!("Tcp listener: {:?}", listener);

    let mut event_loop = EventLoop::new(listener);
    let mut ftpserver = FtpServer::new(&mut event_loop);
    event_loop.run(&mut ftpserver);
}
