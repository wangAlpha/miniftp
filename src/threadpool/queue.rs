use std::collections::VecDeque;
use std::sync::{Arc, Condvar, Mutex};

pub type BlockingQueueRef<T> = Arc<BlockingQueue<T>>;

pub struct BlockingQueue<T> {
    queue: Mutex<VecDeque<T>>,
    condvar: Condvar,
}

impl<T> BlockingQueue<T> {
    pub fn new(capacity: usize) -> Self {
        BlockingQueue {
            queue: Mutex::new(VecDeque::with_capacity(capacity)),
            condvar: Condvar::new(),
        }
    }
    pub fn pop_front(&self) -> T {
        let mut q = self.queue.lock().unwrap();
        loop {
            match q.pop_front() {
                Some(e) => return e,
                None => q = self.condvar.wait(q).unwrap(),
            }
        }
    }
    pub fn push_back(&self, item: T) {
        self.queue.lock().unwrap().push_back(item);
        self.condvar.notify_one();
    }
    pub fn notify_all(&self) {
        self.condvar.notify_all();
    }
    pub fn len(&self) -> usize {
        self.queue.lock().unwrap().len()
    }
}
