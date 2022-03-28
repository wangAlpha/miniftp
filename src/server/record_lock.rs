use nix::fcntl::flock;
use nix::fcntl::FlockArg;

pub struct FileLock {
    fd: i32,
}

impl FileLock {
    pub fn new(fd: i32) -> Self {
        FileLock { fd }
    }
    pub fn lock(&self, writeable: bool) {
        let args = if writeable {
            FlockArg::LockExclusiveNonblock
        } else {
            FlockArg::LockSharedNonblock
        };
        flock(self.fd, args).unwrap();
    }
    pub fn unlock(&self) {
        flock(self.fd, FlockArg::UnlockNonblock).expect("unlock file failed");
    }
}

impl Drop for FileLock {
    fn drop(&mut self) {
        self.unlock();
    }
}
