use crate::handler::session::Session;
use crate::net::connection::{ConnRef, Connection, State};
use crate::net::event_loop::{EventLoop, Handler, Token};
use crate::net::queue::{BlockingQueue, BlockingQueueRef};
use crate::net::sorted_list::TimerList;
use crate::threadpool::threadpool::ThreadPool;
use crate::utils::config::{Config, DEFAULT_CONF_FILE};
use log::{debug, info};
use nix::sys::epoll::EpollFlags;
use nix::unistd::read;
use std::collections::HashMap;
use std::os::unix::prelude::AsRawFd;
use std::sync::{Arc, Mutex};

const DEFAULT_TIME_OUT: u64 = 60; // time (s)

type TaskQueueRef = BlockingQueueRef<(ConnRef, Vec<u8>)>;
pub struct FtpServer {
    conn_list: TimerList<i32, ConnRef>,
    request_queue: TaskQueueRef,
    worker_pool: ThreadPool,
    sessions: Arc<Mutex<HashMap<i32, Session>>>,
}

impl FtpServer {
    pub fn new(config: &Config, event_loop: &mut EventLoop) -> Self {
        let q: TaskQueueRef = BlockingQueueRef::new(BlockingQueue::new(64));
        let pool = ThreadPool::new(0);
        let sessions = Arc::new(Mutex::new(HashMap::<i32, Session>::new()));
        let data_listen_map = Arc::new(Mutex::new(HashMap::<i32, i32>::new()));

        for _ in 0..pool.len() {
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
                        .receive_data(msg, &c, &mut conn_map);
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
            conn_list: TimerList::new(DEFAULT_TIME_OUT),
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
        if let Token::Listen(listen_fd) = token {
            let mut conn = Connection::accept(listen_fd);
            let fd = conn.get_fd();
            debug!("a new connection event fd: {:?}", token);
            conn.register_read(event_loop);
            self.conn_list.insert(fd, Arc::new(Mutex::new(conn)));
        }
    }
    fn notify(&mut self, event_loop: &mut EventLoop, token: Token, revents: EpollFlags) {
        if let Token::Notify(fd) = token {
            if !self.conn_list.contains(&fd) {
                self.conn_list
                    .insert(fd, Arc::new(Mutex::new(Connection::new(fd))));
            }
            let mut conn = self.conn_list.get_mut(&fd).unwrap();

            let state = conn.lock().unwrap().dispatch(revents);
            debug!("A W/R event fd:{} state: {:?}", fd, state);
            if state == State::Finished || state == State::Closed {
                let msg = conn.lock().unwrap().get_msg();
                let c = conn.clone();
                self.request_queue.push_back((c, msg));
                if state == State::Closed {
                    conn.lock().unwrap().deregister(event_loop);
                    self.conn_list.remove(&fd);
                    debug!("disconnection fd: {}", fd);
                }
            }
        } else if let Token::Timer(fd) = token {
            self.conn_list.remove_idle();
            let mut _buf = [0u8; 8];
            read(fd, &mut _buf); // 读取这个 timer_fd
        }
    }
}

pub fn run_server() {
    // daemonize();
    let config = Config::new(DEFAULT_CONF_FILE);
    let addr = format!("{}:{}", config.server_addr, config.server_port);
    let (_, listener) = Connection::bind(&addr);
    info!(
        "Start server listener, addr: {}, fd: {:?}",
        addr,
        listener.as_raw_fd()
    );

    let mut event_loop = EventLoop::new(listener);
    let mut ftpserver = FtpServer::new(&config, &mut event_loop);
    event_loop.run(&mut ftpserver);
}
