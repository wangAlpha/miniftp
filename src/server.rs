use std::{sync::Arc, num::Saturating};
use std::sync::Mutex;
use std::{fmt::format, os::unix::thread};
use std::{net::TcpListener, str::Lines};

use super::queue::BlockingQueue;
use super::queue::BlockingQueueRef;
use log::Log;
use mio::event::Event;
use std::thread::JoinHandle;

struct FtpServer {
    listener: TcpListener,
    conn_list: Slab<ConnRef>,
    request_queue: TaskQueuRef,
    worker: JoinHandle<()>,
}
use std::thread::spawn;

impl FtpServer {
    fn new(listener: TcpListener) -> Self {
        let q = BlockingQueueRef::new(BlockingQueue::new(64));
        let q_clone = q.clone();
        let worker = spawn(|| {
            // consume_task_loop(q_clone);
        });
        FtpServer {
            listener,
            conn_list: todo!(),
            request_queue: todo!(),
            worker: todo!(),
        }
    }
}
impl Handler for FtpServer {
    type Timeout = ();
    // type Message = SenderMsg;
    fn ready(&mut self, event_loop: &mut EventLoop<FtpServer>, token: Token, events: EventSet) {
        match token {
            SEVER => {}
            _ => {}
        }
    }
    fn notify(&mut self, event_loop: &mut EventLoop<FtpServer>, msg: SenderMsg) {
        let (token, curr_state, req_state) msg;
        let mut conn = self.conn_list[token].lock().unwarp();
        match (curr_state, req_state) {
            (State::Writing, State::Writing) => conn.ensure_write_registered(event_loop),
            (State::Writing, State::Finished) => conn.transition_to_finished(event_loop),
            other => panic!("invalid request {:?}", other),
        }
    }
}
type ConnRef = Arc<Mutex<Connection>>;
type SenderMsg = (Token, State, State);
pub struct EventLoop;
impl EventLoop {
    fn new() -> Self {
        EventLoop {}
    }
    fn register(&self) {}
    fn run() {}
}

// mod config;
// use config;
pub fn run_server() {
    // let config = Config::from_cwd_config();
    // let addr_port = config.get
    let addr = format!("0.0.0.0:{}", 7878);
    let listener = TcpListener::bind(&addr).unwrap();
    let mut event_loop = EventLoop::new();

    // event_loop
    //     .register(&listener, SERVER, EventSet::readable(), PollOpt::level())
    //     .unwrap();
    let mut ftpserver = FtpServer::new(listener);
    // event_loop.run(&mut ftpserver).unwrap();
}
