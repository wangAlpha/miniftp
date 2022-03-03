use nix::sys::epoll::{EpollCreateFlags, EpollEvent, EpollFlags};
use std::sync::{Arc, Mutex};

use super::server::EventLoop;
pub type ConnRef = Arc<Mutex<Connection>>;
#[derive(Debug, Eq, PartialEq, Clone)]
enum State {
    Reading,
    Ready,
    Writing,
    Finished,
    Closed,
}
pub struct Connection {
    fd: i32,
    state: State,
    write_buf: Vec<u8>,
    read_buf: Vec<u8>,
    event_addd: bool,
    // TODO: channel
}
impl Connection {
    fn new() -> Self {
        Connection {
            fd: 0,
            state: State::Closed,
            write_buf: Vec::new(),
            read_buf: Vec::new(),
            event_addd: false,
        }
    }
}
const NONE_EVENT: EpollFlags = 0;
const READ_EVENT: EpollFlags = EpollFlags::EEpollFlags::EPOLLIN | EpollFlags::EEpollFlags::EPOLLPRI;
const WRITE_EVENT: EpollFlags = EpollFlags::EEpollFlags::EPOLLOUT;

#[derive(Debug)]
pub struct Channel {
    fd: i32,
    events: EpollEvent,  // User-concerned events
    revents: EpollEvent, // Current events
}

const EVENT_ERROR: EpollFlags = EpollFlags::EPOLLERR;
const EVENT_WRITEABLE: EpollFlags = EpollFlags::EPOLLIN;
const EVENT_READABLE: EpollFlags =
    EpollFlags::EPOLLOUT | EpollFlags::EPOLLPRI | EpollFlags::EPOLLHUP;
