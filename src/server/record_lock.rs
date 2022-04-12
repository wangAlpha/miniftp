use nix::fcntl::{flock, FlockArg};

pub struct FileLock {
    fd: i32,
}

impl FileLock {
    pub fn new(fd: i32) -> Self {
        FileLock { fd }
    }
    pub fn lock(&self, writeable: bool) -> &Self {
        let args = if writeable {
            FlockArg::LockExclusiveNonblock
        } else {
            FlockArg::LockSharedNonblock
        };
        flock(self.fd, args).unwrap();
        self
    }
    pub fn unlock(&self) -> &Self {
        flock(self.fd, FlockArg::UnlockNonblock).expect("unlock file failed");
        self
    }
}

impl Drop for FileLock {
    fn drop(&mut self) {
        self.unlock();
    }
}
