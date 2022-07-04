use super::queue::BlockingQueue;
use super::queue::BlockingQueueRef;
use log::debug;
use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::thread;
use std::time::Instant;

enum Message {
    NewJob(Job),
    OnceJob(Job),
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
        let core_size = thread::available_parallelism().unwrap().get();

        let max_size = core_size * 8;
        let size = if size > 0 { size } else { core_size + 1 };

        let sender = Arc::new(BlockingQueue::new(core_size * 2));
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
        self.sender.push_back(Message::NewJob(job));
    }
    pub fn adjust_workers(&mut self) {
        if self.sender.len() >= self.core_size {
            if  self.workers.len() < self.max_size {
                let size = self.workers.len();
                for i in 0..self.core_size {
                    let id = size + i;
                    self.workers
                        .insert(id, Worker::new(id, self.sender.clone()));
                    debug!("Start a new worker: {}", id);
                }
                // Expansion condition
                self.sender.notify_all();
            }
        } //else if self.sender.len() <= self.core_size && self.workers.len() >= self.core_size * 2 {
          //  // Dynamic shrink
          //  // thread::current().id()
          //  let mut count = 1;
          //  for worker in self.workers.iter() {
          //      if !worker.is_running() {
          //          count += 1;
          //      }
          //  }
          //  while count > 0 {
          //      
          //  }
          // }
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
       // for worker in self.workers.iter() {
       //     if let thread = worker.thread {
       //         thread.join().unwrap();
       //     }
       // }
        debug!("Shutting down {} worker", self.workers.len());
    }
}

struct Worker {
    id: usize,
    thread: thread::JoinHandle<()>,
    running: AtomicBool,
    idle_time: Instant,
}

impl Worker {
    fn new(id: usize, receiver: BlockingQueueRef<Message>) -> Worker {
        let thread = thread::spawn(move || loop {
            let message = receiver.pop_front();
            match message {
                Message::NewJob(job) => {
                    job();
                }
                Message::OnceJob(job)=>{
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
            thread: thread,
            running: AtomicBool::new(true),
            idle_time: Instant::now(),
        }
    }
    //fn is_running(&self) ->bool {
    //    self.thread.is_running()
    //}
    //pub fn idle_time(&self) -> Instant {
    //    self.idle_time
    //}
}
