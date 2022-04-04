use log::warn;
use nix::fcntl::fcntl;
use nix::fcntl::FcntlArg;
use nix::sys::epoll::{epoll_create1, epoll_ctl, epoll_wait};
use nix::sys::epoll::{EpollCreateFlags, EpollEvent, EpollFlags, EpollOp};
use nix::unistd::close;

const EVENT_SIZE: usize = 1024;

#[derive(Debug, Clone)]
pub struct Poller {
    events: Vec<EpollEvent>,
    poll_fd: i32,
}

impl Poller {
    pub fn new() -> Self {
        let poll_fd = epoll_create1(EpollCreateFlags::EPOLL_CLOEXEC).unwrap();
        let events = vec![EpollEvent::new(EpollFlags::empty(), 0); EVENT_SIZE];
        Poller { events, poll_fd }
    }
    pub fn register(&mut self, listen_fd: i32, interest: EpollFlags) {
        let mut event = EpollEvent::new(interest, listen_fd as u64);
        epoll_ctl(self.poll_fd, EpollOp::EpollCtlAdd, listen_fd, &mut event).unwrap();
    }
    pub fn poll(&mut self) -> usize {
        let num_events = epoll_wait(self.poll_fd, &mut self.events, -1).unwrap();
        if num_events == 0 {
            warn!("Nothing happened");
        } else if num_events == self.events.len() {
            let events = vec![EpollEvent::new(EpollFlags::empty(), 0); self.events.len()];
            self.events.extend(events.iter());
        }
        num_events
    }
    pub fn update(&self, op: EpollOp, fd: i32, event: &mut Option<EpollEvent>) {
        epoll_ctl(self.poll_fd, op, fd, event).unwrap();
    }
    pub fn event(&self, i: usize) -> (i32, EpollEvent) {
        let event = self.events[i];
        (event.data() as i32, event)
    }
}

impl Drop for Poller {
    fn drop(&mut self) {
        if 0 > fcntl(self.poll_fd, FcntlArg::F_GETFL).unwrap() {
            close(self.poll_fd).unwrap();
        }
    }
}
