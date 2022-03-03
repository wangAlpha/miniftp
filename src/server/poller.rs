use super::connection::Channel;
use nix::sys::epoll::EpollCreateFlags;
use nix::sys::epoll::{epoll_create, epoll_create1, epoll_ctl, epoll_wait};
use nix::sys::epoll::{EpollEvent, EpollFlags, EpollOp};
use nix::sys::sendfile;
use nix::sys::socket::setsockopt;
use nix::unistd::{close, read, write};
use std::collections::HashMap;
use std::os::unix::prelude::AsRawFd;
use std::sync::Arc;
use std::{net::TcpListener, sync::mpsc::channel};

const EVENT_SIZE: usize = 1024;

#[derive(Debug)]
pub struct Poller {
    events: Vec<EpollEvent>,
    poll_fd: i32,
}

impl Poller {
    pub fn new() -> Self {
        let mut events = std::iter::repeat_with(|| EpollEvent::new(EpollFlags::empty(), 0))
            .take(EVENT_SIZE)
            .collect::<Vec<_>>();
        Poller {
            events,
            poll_fd: -1,
        }
    }
    pub fn register(&mut self, listener: TcpListener, interest: EpollFlags) {
        let sockfd = listener.as_raw_fd();
        let poll_fd = epoll_create1(EpollCreateFlags::EPOLL_CLOEXEC).unwrap();
        let mut event = EpollEvent::new(interest, sockfd as u64);
        epoll_ctl(poll_fd, EpollOp::EpollCtlAdd, sockfd, &mut event).unwrap();
        self.poll_fd = poll_fd;
    }
    pub fn poll(&mut self) -> usize {
        let num_events = epoll_wait(self.poll_fd, &mut self.events, -1).unwrap();
        if num_events == 0 {
            println!("Nothing happened");
        } else if num_events == self.events.len() {
            // self.events.extend()
        }
        num_events
    }
    pub fn event(&self, i: usize) -> (i32, EpollEvent) {
        let event = self.events[i];
        (event.data() as i32, event.events())
    }
    pub fn update(&self, op: EpollFlags, token: i32, event: EpollFlags) {
        epoll_ctl(self.poll_fd, op, token, event);
    }
}
