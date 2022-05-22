use nix::fcntl::{flock, FlockArg};

pub struct FileLock {
    fd: i32,
    is_drop: bool,
}

impl FileLock {
    pub fn new(fd: i32) -> Self {
        FileLock { fd, is_drop: true }
    }
    pub fn set_drop(&mut self, on: bool) {
        self.is_drop = on;
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
        if self.is_drop {
            self.unlock();
        }
    }
}
