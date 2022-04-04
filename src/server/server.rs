use crate::net::connection::{ConnRef, Connection, State};
use crate::net::event_loop::{EventLoop, Handler, Token};
use crate::net::queue::{BlockingQueue, BlockingQueueRef};
use crate::threadpool::threadpool::ThreadPool;
use crate::{
    handler::session::Session,
    utils::config::{Config, DEFAULT_CONF_FILE},
};
use log::{debug, info};
use nix::fcntl::{flock, open, FlockArg, OFlag};
use nix::libc::exit;
use nix::sys::epoll::EpollFlags;
use nix::sys::resource::*;
use nix::sys::signal::{pthread_sigmask, signal};
use nix::sys::signal::{SigHandler, SigSet, SigmaskHow, Signal};
use nix::sys::stat::{umask, Mode};
use nix::unistd::{chdir, fork, ftruncate, getpid, setsid, write};
use num_cpus;
use std::collections::HashMap;
use std::os::unix::prelude::AsRawFd;
use std::sync::{Arc, Mutex};

const LOCK_FILE: &str = "/var/run/miniftp.pid";

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

type TaskQueueRef = BlockingQueueRef<(ConnRef, Vec<u8>)>;
pub struct FtpServer {
    conn_list: HashMap<i32, ConnRef>,
    request_queue: TaskQueueRef,
    worker_pool: ThreadPool,
    sessions: Arc<Mutex<HashMap<i32, Session>>>,
}

impl FtpServer {
    pub fn new(event_loop: &mut EventLoop) -> Self {
        let q: TaskQueueRef = BlockingQueueRef::new(BlockingQueue::new(64));
        let pool = ThreadPool::new(0);
        let sessions = Arc::new(Mutex::new(HashMap::<i32, Session>::new()));
        let data_listen_map = Arc::new(Mutex::new(HashMap::<i32, i32>::new()));
        let config = Config::new(DEFAULT_CONF_FILE);

        for _ in 0..num_cpus::get() {
            let q_clone = q.clone();
            let event_loop = event_loop.clone();
            let sessions = sessions.clone();
            let mut conn_map = data_listen_map.clone();
            let config = config.clone();
            pool.execute(move || loop {
                let (conn, msg) = q_clone.pop_front();
                let fd = conn.lock().unwrap().get_fd();
                // TODO: how to clean conn_map
                if conn_map.lock().unwrap().contains_key(&fd) {
                    let data_fd = conn.lock().unwrap().get_fd();
                    let cmd_fd = conn_map.lock().unwrap()[&data_fd];
                    let c = conn.clone();
                    sessions
                        .lock()
                        .unwrap()
                        .get_mut(&cmd_fd)
                        .unwrap()
                        .receive_data(msg, &c);
                } else {
                    let cmd_fd = fd;
                    if !sessions.lock().unwrap().contains_key(&cmd_fd) {
                        let s = Session::new(&config, conn, event_loop.clone());
                        sessions.lock().unwrap().insert(cmd_fd, s);
                    }
                    sessions
                        .lock()
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
        match token {
            Token::Listen(listen_fd) => {
                let mut conn = Connection::accept(listen_fd);
                let fd = conn.get_fd();
                debug!("a new connection event fd: {:?}", token);
                conn.register_read(event_loop);
                self.conn_list.insert(fd, Arc::new(Mutex::new(conn)));
            }
            Token::Read(fd) => {
                self.conn_list
                    .entry(fd)
                    .or_insert(Arc::new(Mutex::new(Connection::new(fd))));
                // ugly clone
                let state = self.conn_list[&fd].lock().unwrap().dispatch(revents);
                debug!("A W/R event fd:{} state: {:?}", fd, state);
                if state == State::Finished || state == State::Closed {
                    let msg = self.conn_list[&fd].lock().unwrap().get_msg();
                    if !msg.is_empty() {
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
    }
    fn notify(&mut self, event_loop: &mut EventLoop, token: Token, revents: EpollFlags) {}
}

pub fn run_server() {
    // daemonize();
    let addr = "0.0.0.0:8089";
    let (_, listener) = Connection::bind(addr);
    info!(
        "Start sever listener, addr: {}, fd: {:?}",
        addr,
        listener.as_raw_fd()
    );

    let mut event_loop = EventLoop::new(listener);
    let mut ftpserver = FtpServer::new(&mut event_loop);
    event_loop.run(&mut ftpserver);
}
