use super::connection::{ConnRef, Connection, State};
use super::poller::Poller;
use super::queue::{BlockingQueue, BlockingQueueRef};
use crate::handler::session::{self, Session};
use crate::threadpool::threadpool::ThreadPool;
use log::debug;
use nix::fcntl::{flock, open, FlockArg, OFlag};
use nix::libc::exit;
use nix::sys::epoll::{EpollEvent, EpollFlags, EpollOp};
use nix::sys::resource::*;
use nix::sys::signal::{pthread_sigmask, signal};
use nix::sys::signal::{SigHandler, SigSet, SigmaskHow, Signal};
use nix::sys::stat::{umask, Mode};
use nix::unistd::{chdir, fork, ftruncate, getpid, setsid, write};
use num_cpus;
use std::collections::HashSet;
use std::net::TcpListener;
use std::os::unix::prelude::AsRawFd;
use std::sync::Arc;
use std::sync::{Mutex, RwLock};
use std::{collections::HashMap, fmt::Debug};

pub const EVENT_LEVEL: EpollFlags = EpollFlags::EPOLLET;
pub const EVENT_READ: EpollFlags = EpollFlags::EPOLLIN;
pub const EVENT_ERR: EpollFlags = EpollFlags::EPOLLERR;
pub const EVENT_HUP: EpollFlags = EpollFlags::EPOLLHUP;

const LOCK_FILE: &str = "/var/run/miniftp.pid";

// fd, and listen fd
#[derive(Debug, Clone, Copy)]
pub enum ConTyp {
    Cmd,
    Data,
}
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

pub fn daemonize() {
    umask(Mode::empty());
    getrlimit(Resource::RLIMIT_NOFILE).expect("get trlimit failed!");
    let result = unsafe { fork().expect("cant't fork a new process") };
    if result.is_parent() {
        unsafe { exit(0) };
    }
    unsafe {
        signal(Signal::SIGPIPE, SigHandler::SigIgn).unwrap();
        signal(Signal::SIGHUP, SigHandler::SigIgn).unwrap();
    }
    pthread_sigmask(SigmaskHow::SIG_BLOCK, Some(&SigSet::all()), None).unwrap();
    setsid().expect("can't set sid");
    chdir("/").unwrap();

    // let fd0 = open("/dev/null", OFlag::O_RDWR, Mode::empty()).unwrap();
    // let fd1 = dup(0).unwrap();
    // let fd2 = dup(0).unwrap();
    // if fd0 != 0 || fd1 != 1 || fd2 != 2 {
    //     error!("unexpected file descriptors {} {} {}", fd0, fd1, fd2);
    //     unsafe {
    //         exit(1);
    //     }
    // }
}

pub fn already_runing() -> bool {
    let lock_mode = Mode::S_IRUSR | Mode::S_IWUSR | Mode::S_IRGRP | Mode::S_IROTH;
    let fd = open(LOCK_FILE, OFlag::O_RDWR | OFlag::O_CREAT, lock_mode).unwrap();
    flock(fd, FlockArg::LockExclusiveNonblock).unwrap();
    ftruncate(fd, 0).unwrap();
    let pid = getpid();
    let buf = format!("{}", pid);
    match write(fd, buf.as_bytes()) {
        Ok(0) | Err(_) => return false,
        _ => return true,
    }
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
// pub enum Mode {}

type TaskQueueRef = BlockingQueueRef<(ConnRef, Vec<u8>)>;
pub struct FtpServer {
    conn_list: HashMap<i32, ConnRef>,
    request_queue: TaskQueueRef,
    worker_pool: ThreadPool,
    sessions: Arc<RwLock<HashMap<i32, Session>>>,
}

impl FtpServer {
    pub fn new(event_loop: &mut EventLoop) -> Self {
        let q: TaskQueueRef = BlockingQueueRef::new(BlockingQueue::new(64));
        let pool = ThreadPool::new(num_cpus::get());
        let sessions = Arc::new(RwLock::new(HashMap::<i32, Session>::new()));
        let data_listen_map = Arc::new(Mutex::new(HashMap::<i32, i32>::new()));
        for _ in 0..num_cpus::get() {
            let q_clone = q.clone();
            let event_loop = event_loop.clone();
            let sessions = sessions.clone();
            let mut conn_map = data_listen_map.clone();
            pool.execute(move || loop {
                let (conn, msg) = q_clone.pop_front();
                let fd = conn.lock().unwrap().get_fd();
                if conn_map.lock().unwrap().contains_key(&fd) {
                    let data_fd = conn.lock().unwrap().get_fd();
                    let cmd_fd = conn_map.lock().unwrap()[&data_fd];

                    let c = conn.lock().unwrap().clone();
                    sessions
                        .write()
                        .unwrap()
                        .get_mut(&cmd_fd)
                        .unwrap()
                        .receive_data(msg, c);
                } else {
                    let cmd_fd = fd;
                    if !sessions.read().unwrap().contains_key(&cmd_fd) {
                        let s = Session::new(conn, event_loop.clone());
                        sessions.write().unwrap().insert(cmd_fd, s);
                    }
                    sessions
                        .write()
                        .unwrap()
                        .get_mut(&cmd_fd)
                        .unwrap()
                        .handle_command(msg, &mut conn_map);
                }
            });
        }
        FtpServer {
            conn_list: HashMap::new(),
            request_queue: q,
            worker_pool: pool,
            sessions,
        }
    }
}

impl Handler for FtpServer {
    type Message = String;
    type Timeout = i32;
    fn ready(&mut self, event_loop: &mut EventLoop, token: Token, revents: EpollFlags) {
        debug!("a new event token: {:?}", token);
        match token {
            Token::Listen(listen_fd) => {
                let mut conn = Connection::accept(listen_fd);
                let fd = conn.get_fd();
                conn.register_read(event_loop);
                self.conn_list.insert(fd, Arc::new(Mutex::new(conn)));
            }
            Token::Read(fd) => {
                self.conn_list
                    .entry(fd)
                    .or_insert(Arc::new(Mutex::new(Connection::new(fd))));
                let state = self.conn_list[&fd].lock().unwrap().dispatch(revents);
                if state == State::Finished {
                    let msg = self.conn_list[&fd].lock().unwrap().get_msg();
                    let clone = self.conn_list[&fd].clone();
                    self.request_queue.push_back((clone, msg));
                }
                if state == State::Closed {
                    self.conn_list[&fd].lock().unwrap().deregister(event_loop);
                    self.conn_list.remove(&fd);
                    debug!("disconnection fd: {}", fd);
                }
            }
        }
    }
    fn notify(&mut self, event_loop: &mut EventLoop, token: Token, revents: EpollFlags) {}
}

pub fn run_server() {
    // daemonize();
    let (_, listener) = Connection::bind("0.0.0.0:8089");
    debug!("Tcp listener: {:?}", listener);

    let mut event_loop = EventLoop::new(listener);
    let mut ftpserver = FtpServer::new(&mut event_loop);
    event_loop.run(&mut ftpserver);
}
