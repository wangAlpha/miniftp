use super::poller::Poller;
use super::socket::Socket;
use nix::sys::epoll::{EpollEvent, EpollFlags, EpollOp};
use nix::sys::time::{TimeSpec, TimeValLike};
use nix::sys::timerfd::{ClockId, Expiration, TimerFd, TimerFlags, TimerSetTimeFlags};
use std::collections::{HashMap, HashSet};
use std::os::unix::prelude::AsRawFd;
use std::sync::{Arc, Mutex};

pub const EVENT_LEVEL: EpollFlags = EpollFlags::EPOLLET;
pub const EVENT_READ: EpollFlags = EpollFlags::EPOLLIN;
pub const EVENT_ERR: EpollFlags = EpollFlags::EPOLLERR;
pub const EVENT_HUP: EpollFlags = EpollFlags::EPOLLHUP;
pub const EVENT_WRIT: EpollFlags = EpollFlags::EPOLLOUT;

#[derive(Debug, Clone, Copy)]
pub enum Token {
    Listen(i32),
    Notify(i32),
    Timer(i32),
}

pub trait Handler: Sized {
    type Timeout;
    type Message;
    fn ready(&mut self, event_loop: &mut EventLoop, token: Token);
    fn notify(&mut self, event_loop: &mut EventLoop, token: Token, revent: EpollFlags);
}

#[derive(Debug, Clone)]
pub struct EventLoop {
    listener: Arc<Socket>,
    listeners: Arc<Mutex<HashSet<i32>>>,
    timers: Arc<Mutex<HashMap<i32, TimerFd>>>,
    poller: Poller,
    run: bool,
}

impl EventLoop {
    pub fn new(listener: Socket) -> Self {
        let mut poller = Poller::new();
        let interest = EVENT_READ|EVENT_LEVEL;
        poller.register(listener.as_raw_fd(), interest);
        EventLoop {
            listener: Arc::new(listener),
            listeners: Arc::new(Mutex::new(HashSet::new())),
            timers: Arc::new(Mutex::new(HashMap::new())),
            run: true,
            poller,
        }
    }
    pub fn register(&mut self, listener: Socket, interest: EpollFlags) {
        let fd = listener.as_raw_fd();
        // self.listeners.lock().unwrap().insert(listener);
        self.poller.register(fd, interest);
    }
    pub fn register_listen(&mut self, listener: Socket) {
        self.register(listener, EVENT_HUP | EVENT_WRIT|  EVENT_READ | EVENT_LEVEL);
    }
    pub fn reregister(&self, fd: i32, interest: EpollFlags) {
        let event = EpollEvent::new(interest, fd as u64);
        self.poller
            .update(EpollOp::EpollCtlAdd, fd, &mut Some(event));
    }
    pub fn deregister(&self, fd: i32) {
        self.poller.update(EpollOp::EpollCtlDel, fd, &mut None);
    }
    fn is_listen_event(&self, fd: i32) -> bool {
        self.listener.as_raw_fd() == fd
        //|| self.listeners.lock().unwrap().contains_key(&fd)
    }
    fn is_timer_event(&self, fd: i32) -> bool {
        self.timers.lock().unwrap().contains_key(&fd)
    }
    pub fn add_timer(&mut self, interval: i64) {
        let timer_fd = TimerFd::new(
            ClockId::CLOCK_MONOTONIC,
            TimerFlags::TFD_CLOEXEC | TimerFlags::TFD_NONBLOCK,
        )
        .unwrap();

        timer_fd
            .set(
                Expiration::IntervalDelayed(
                    TimeSpec::seconds(interval),
                    TimeSpec::seconds(interval),
                ),
                TimerSetTimeFlags::empty(),
            )
            .unwrap();
        self.poller
            .register(timer_fd.as_raw_fd(), EVENT_READ | EVENT_LEVEL);
        self.timers
            .lock()
            .unwrap()
            .insert(timer_fd.as_raw_fd(), timer_fd);
    }
    pub fn run<H>(&mut self, handler: &mut H)
    where
        H: Handler,
    {
        while self.run {
            let cnt = self.poller.poll();
            let mut ready_channels = Vec::new();
            let mut notify_channels = Vec::new();
            let mut timer_channels = Vec::new();
            for i in 0..cnt {
                let (fd, event) = self.poller.event(i);
                if self.is_listen_event(fd) {
                    ready_channels.push(Token::Listen(fd));
                } else if self.is_timer_event(fd) {
                    timer_channels.push((Token::Timer(fd), event));
                } else {
                    notify_channels.push((Token::Notify(fd), event));
                };
            }
            // io ready event: listen event
            for &token in ready_channels.iter() {
                handler.ready(self, token);
            }
            // io read and write event
            for &(token, event) in notify_channels.iter() {
                handler.notify(self, token, event.events());
            }
            let mut _buf = [0u8; 8];
            for &(token, event) in timer_channels.iter() {
                handler.notify(self, token, event.events());
            }
        }
    }
}
