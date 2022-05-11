use crate::handler::session::Session;
use crate::net::acceptor::Acceptor;
use crate::net::connection::{Connection, EventSet};
use crate::net::event_loop::{EventLoop, Handler, Token};
use crate::net::queue::{BlockingQueue, BlockingQueueRef};
use crate::net::socket::Socket;
use crate::net::sorted_list::TimerList;
use crate::threadpool::threadpool::ThreadPool;
use crate::utils::config::Config;
use crate::utils::utils::{already_running, daemonize};
use log::{debug, info, warn};
use nix::sys::epoll::EpollFlags;
use nix::unistd::read;
use num_cpus;
use std::sync::{Arc, Mutex};
use std::{os::unix::prelude::AsRawFd, path::PathBuf};

const DEFAULT_TIME_OUT: u64 = 90; // time (s)
const DEFAULT_TIMER: i64 = 2;
type TaskQueueRef = BlockingQueueRef<Arc<Mutex<Session>>>;

pub struct FtpServer {
    request_queue: TaskQueueRef,
    worker_pool: ThreadPool,
    sessions: TimerList<i32, Arc<Mutex<Session>>>, // <cmd_fd, session_ref>
    event_loop: EventLoop,
    config: Config,
}

impl FtpServer {
    pub fn new(config: Config, event_loop: &mut EventLoop) -> Self {
        let q: TaskQueueRef = BlockingQueueRef::new(BlockingQueue::new(64));
        let pool = ThreadPool::new(num_cpus::get() * 2);
        event_loop.add_timer(5);
        for _ in 0..pool.len() {
            let q_clone = q.clone();
            pool.execute(move || loop {
                let session = q_clone.pop_front();
                session.lock().unwrap().handle_command();
            });
        }
        FtpServer {
            request_queue: q,
            worker_pool: pool,
            sessions: TimerList::new(DEFAULT_TIME_OUT),
            event_loop: event_loop.clone(),
            config,
        }
    }
}

impl Handler for FtpServer {
    type Message = String;
    type Timeout = i32;
    // Handling connection events
    fn ready(&mut self, event_loop: &mut EventLoop, token: Token) {
        if let Token::Listen(listen_fd) = token {
            let sock = Acceptor::accept(listen_fd);
            let mut conn = Connection::new(sock.clone());

            debug!("A new connection: {:?}:{}", token, sock.as_raw_fd());

            if self.config.max_clients > self.sessions.len() {
                conn.register_read(event_loop);
                let s = Session::new(&self.config, conn, event_loop);
                self.sessions
                    .insert(sock.as_raw_fd(), Arc::new(Mutex::new(s)));
            } else {
                warn!(
                    "Max client number: {}, Session number: {}, shutdown conn: {}",
                    self.config.max_clients,
                    self.sessions.len(),
                    sock.as_raw_fd()
                );
                conn.shutdown();
            }
        }
    }
    // Handling IO and timer events
    fn notify(&mut self, event_loop: &mut EventLoop, token: Token, revents: EpollFlags) {
        if let Token::Notify(fd) = token {
            if let Some(s) = self.sessions.get(&fd) {
                s.lock().unwrap().set_revents(&revents);
                debug!("Connection: {}, revents: {:?}", fd, revents);
                if revents.is_close() || revents.is_hup() {
                    self.sessions.remove(&fd);
                    event_loop.deregister(fd);
                    debug!("Remove session: {}", fd);
                } else {
                    self.request_queue.push_back(s.clone());
                }
            } else {
                event_loop.deregister(fd);
            }
        } else if let Token::Timer(fd) = token {
            // Log out of some idle sessions.
            let old_len = self.sessions.len();
            self.sessions.remove_idle();
            let new_len = self.sessions.len();
            if old_len != new_len {
                debug!("Remove idle session, new len: {}", new_len);
            }
            let mut _buf = [0u8; 8];
            // Read this timer_fd otherwise repeated events are triggered.
            read(fd, &mut _buf).unwrap_or_default();
        }
    }
}
pub fn run_server(config: &PathBuf) {
    if already_running() {
        warn!("Already running...");
        return;
    }
    daemonize();

    let config = Config::new(&config);
    debug!("config: {:?}", config);
    let addr = format!("{}:{}", config.server_addr, config.server_port);
    info!("Start server listen, addr: {}", addr);
    let listener = Socket::bind(&addr);
    let mut event_loop = EventLoop::new(listener);
    let mut ftpserver = FtpServer::new(config, &mut event_loop);
    event_loop.run(&mut ftpserver);
}
