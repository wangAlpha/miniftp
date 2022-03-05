use super::connection::{ConnRef, Connection};
use super::poller::Poller;
use super::queue::{BlockingQueue, BlockingQueueRef};
use crate::threadpool::threadpool::ThreadPool;
use log::{debug, info, warn};
use nix::sys::epoll::{EpollEvent, EpollFlags, EpollOp};
use nix::sys::signal::signal;
use nix::sys::signal::{SigHandler, Signal};
use num_cpus;
use std::collections::HashMap;
use std::net::TcpListener;
use std::os::unix::prelude::AsRawFd;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

// EPOLLIN | EPOLLET
const EVENT_LEVEL: EpollFlags = EpollFlags::EPOLLET;
const EVENT_READ: EpollFlags = EpollFlags::EPOLLIN;
const EVENT_ERR: EpollFlags = EpollFlags::EPOLLERR;
const EVENT_HUP: EpollFlags = EpollFlags::EPOLLHUP;

pub trait Handler: Sized {
    type Timeout;
    type Message;
    fn ready(&mut self, event_loop: &mut EventLoop, fd: i32, revent: EpollEvent);
    fn notify(&mut self, event_loop: &mut EventLoop, fd: i32, revent: EpollEvent);
}

pub fn signal_ignore() {
    let signal_set = [Signal::SIGPIPE, Signal::SIGINT];
    for sig in signal_set {
        unsafe {
            signal(sig, SigHandler::SigIgn).unwrap();
        }
    }
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
            // io ready event: connection
            for i in 0..cnt {
                let (fd, revent) = self.poller.event(i);
                handler.ready(self, fd, revent);
            }
            // io read and write event
            for i in 0..cnt {
                let (fd, revent) = self.poller.event(i);
                handler.notify(self, fd, revent);
            }
        }
    }
}
// pub enum Mode {}

type TaskQueueRef = BlockingQueueRef<(String, ConnRef)>;
pub struct FtpServer {
    conn_list: HashMap<i32, ConnRef>,
    listener: TcpListener,
    poller_fd: i32,
    request_queue: TaskQueueRef,
    worker_pool: ThreadPool,
}

impl FtpServer {
    pub fn new(listener: TcpListener) -> Self {
        let q = BlockingQueueRef::new(BlockingQueue::new(64));
        let pool = ThreadPool::new(num_cpus::get());

        FtpServer {
            conn_list: HashMap::new(),
            listener,
            poller_fd: -1,
            worker_pool: pool,
            request_queue: q,
        }
    }
    pub fn execute(&mut self) {
        loop {
            let (msg, conn) = self.request_queue.pop_front();
            println!(
                "handle FTP command {:?} from conn {}",
                msg,
                conn.lock().unwrap().fd()
            );
            conn.lock().unwrap().send("FTP cmd result".as_bytes());
            // process_table_command
            // conn.lock().unwrap()
            // handler task
            // send
        }
    }
}

impl Handler for FtpServer {
    type Message = String;
    type Timeout = i32;
    fn ready(&mut self, event_loop: &mut EventLoop, fd: i32, events: EpollEvent) {
        let listen_fd = self.listener.as_raw_fd();
        let revents = events.events();
        if fd == listen_fd {
            assert_ne!((revents & EVENT_READ).bits(), 0);
            let (fd, conn) = Connection::accept(listen_fd);
            self.conn_list.insert(fd, Arc::new(Mutex::new(conn)));
            self.conn_list[&fd]
                .lock()
                .unwrap()
                .register_read(event_loop);
            debug!("register a new connection: {}", fd);
        } else {
            if (revents & EVENT_READ).bits() > 0 {
                let len = self.conn_list[&fd].lock().unwrap().read();
                if len == 0 {
                    self.conn_list[&fd].lock().unwrap().deregister(event_loop);
                    self.conn_list.remove(&fd);
                    debug!("remove a connection: {}", fd);
                }
            } else if (revents & (EVENT_ERR | EVENT_HUP)).bits() > 0 {
                self.conn_list[&fd].lock().unwrap().deregister(event_loop);
                self.conn_list.remove(&fd);
                debug!("disconnection fd: {}", fd);
            }
        }
    }
    fn notify(&mut self, event_loop: &mut EventLoop, fd: i32, revents: EpollEvent) {
    
    }
}

pub fn run_server() {
    let listener = TcpListener::bind("0.0.0.0:8089").unwrap();
    let listen_fd = listener.as_raw_fd();
    debug!("Tcp listener: {:?}", listener);

    let mut event_loop = EventLoop::new();
    event_loop.register(listen_fd, EVENT_ERR | EVENT_READ | EVENT_LEVEL);
    let mut ftpserver = FtpServer::new(listener);
    event_loop.run(&mut ftpserver);
}
