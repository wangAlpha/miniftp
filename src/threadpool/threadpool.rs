use super::queue::BlockingQueue;
use super::queue::BlockingQueueRef;
use log::debug;
use num_cpus;
use std::collections::HashMap;
use std::sync::Arc;
use std::thread;

enum Message {
    NewJob(Job),
    TimerJob(Job),
    Terminate,
}
pub struct ThreadPool {
    workers: HashMap<usize, Worker>,
    sender: BlockingQueueRef<Message>,
    core_size: usize,
    max_size: usize,
}
type Job = Box<dyn FnOnce() + Send + 'static>;

impl ThreadPool {
    pub fn new(size: usize) -> ThreadPool {
        let core_size = num_cpus::get();
        let max_size = core_size * 8;
        let size = if size > 0 { size } else { core_size + 1 };

        let sender = Arc::new(BlockingQueue::new(num_cpus::get() * 2));
        let receiver = sender.clone();

        let mut workers = HashMap::new();
        for id in 0..size {
            workers.insert(id, Worker::new(id, receiver.clone()));
        }
        debug!("Start {} worker.", workers.len());
        ThreadPool {
            workers,
            sender,
            core_size,
            max_size,
        }
    }

    pub fn execute<F>(&mut self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let job = Box::new(f);
        // Expansion condition
        if self.sender.len() >= self.core_size && self.workers.len() < self.max_size {
            let size = self.workers.len();
            for i in 0..self.core_size {
                let id = size + i;
                self.workers
                    .insert(id, Worker::new(id, self.sender.clone()));
                debug!("Start a new worker: {}", id);
            }
        }
        self.sender.push_back(Message::NewJob(job));
    }

    pub fn len(&self) -> usize {
        self.workers.len()
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        debug!("Sending terminate message to all workers.");

        for _ in &self.workers {
            self.sender.push_back(Message::Terminate);
        }
        for worker in self.workers.values_mut() {
            if let Some(thread) = worker.thread.take() {
                thread.join().unwrap();
            }
        }
        debug!("Shutting down {} worker", self.workers.len());
    }
}

struct Worker {
    id: usize,
    thread: Option<thread::JoinHandle<()>>,
}

impl Worker {
    fn new(id: usize, receiver: BlockingQueueRef<Message>) -> Worker {
        let thread = thread::spawn(move || loop {
            let message = receiver.pop_front();
            match message {
                Message::NewJob(job) => {
                    job();
                }
                Message::TimerJob(job) => {
                    job();
                    break;
                }
                Message::Terminate => {
                    break;
                }
            }
        });
        Worker {
            id,
            thread: Some(thread),
        }
    }
}
