use crate::handler::session::Session;
use crate::net::connection::Connection;
use crate::net::connection::EventSet;
use crate::net::event_loop::{EventLoop, Handler, Token};
use crate::net::queue::{BlockingQueue, BlockingQueueRef};
use crate::net::sorted_list::TimerList;
use crate::threadpool::threadpool::ThreadPool;
use crate::utils::config::{Config, DEFAULT_CONF_FILE};
use crate::utils::utils::{already_running, daemonize};
use log::{debug, info, warn};
use nix::sys::epoll::EpollFlags;
use nix::unistd::read;
use std::collections::HashMap;
use std::os::unix::prelude::AsRawFd;
use std::sync::{Arc, Mutex};

const DEFAULT_TIME_OUT: u64 = 30; // time (s)
const DEFAULT_TIMER: i64 = 2;
type TaskQueueRef = BlockingQueueRef<Arc<Mutex<Session>>>;

pub struct FtpServer {
    request_queue: TaskQueueRef,
    worker_pool: ThreadPool,
    conn_list: HashMap<i32, Arc<Mutex<Connection>>>,
    sessions: TimerList<i32, Arc<Mutex<Session>>>, // <cmd_fd, session_ref>
    event_loop: EventLoop,
    conn_map: Arc<Mutex<HashMap<i32, i32>>>, // <cmd_fd, data fd>
    config: Config,
}

impl FtpServer {
    pub fn new(config: Config, event_loop: &mut EventLoop) -> Self {
        let q: TaskQueueRef = BlockingQueueRef::new(BlockingQueue::new(64));
        let pool = ThreadPool::new(0);
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
            conn_list: HashMap::new(),
            sessions: TimerList::new(DEFAULT_TIME_OUT),
            event_loop: event_loop.clone(),
            conn_map: Arc::new(Mutex::new(HashMap::new())),
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
            let mut conn = Connection::accept(listen_fd);
            let fd = conn.get_fd();
            debug!("A new connection: {:?}:{}", token, fd);

            if self.config.max_clients > self.sessions.len() {
                conn.register_read(event_loop);
                let s = Session::new(&self.config, conn, event_loop, &self.conn_map);
                self.sessions.insert(fd, Arc::new(Mutex::new(s)));
            } else {
                warn!(
                    "Session number: {}, shutdown conn: {}",
                    self.sessions.len(),
                    fd
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
            // 注销一些最不活跃的session
            let old_len = self.sessions.len();
            // self.sessions.remove_idle();
            let new_len = self.sessions.len();
            if old_len != new_len {
                debug!(
                    "Remove idle connection, old len:{}, new len: {}",
                    old_len, new_len
                );
            }
            let mut _buf = [0u8; 8];
            read(fd, &mut _buf).unwrap_or_default(); // 读取这个 timer_fd
        }
    }
}
pub fn run_server() {
    if already_running() {
        warn!("Already running...");
        return;
    }
    // daemonize();

    let config = Config::new(DEFAULT_CONF_FILE);
    debug!("config: {:?}", config);
    let addr = format!("{}:{}", config.server_addr, config.server_port);
    let (_, listener) = Connection::bind(&addr);
    info!(
        "Start server listener, addr: {}, fd: {:?}",
        addr,
        listener.as_raw_fd()
    );

    let mut event_loop = EventLoop::new(listener);
    let mut ftpserver = FtpServer::new(config, &mut event_loop);
    event_loop.run(&mut ftpserver);
}
