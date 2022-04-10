use nix::fcntl::{flock, open, FlockArg, OFlag};
use nix::libc::exit;
use nix::sys::resource::*;
use nix::sys::signal::{pthread_sigmask, signal};
use nix::sys::signal::{SigHandler, SigSet, SigmaskHow, Signal};
use nix::sys::stat::{lstat, umask, Mode, SFlag};
use nix::unistd::{chdir, fork, ftruncate, getpid, setsid, write};

const LOCK_FILE: &'static str = "/var/run/miniftp.pid";

pub fn is_regular(path: &str) -> bool {
    let stat = lstat(path).unwrap();
    stat.st_mode & SFlag::S_IFREG.bits() == SFlag::S_IFREG.bits()
}
pub fn is_link(path: &str) -> bool {
    let stat = lstat(path).unwrap();
    stat.st_mode & SFlag::S_IFLNK.bits() == SFlag::S_IFLNK.bits()
}

pub fn is_dir(path: &str) -> bool {
    let stat = lstat(path).unwrap();
    stat.st_mode & SFlag::S_IFDIR.bits() == SFlag::S_IFDIR.bits()
}

pub fn daemonize() {
    umask(Mode::empty());
    getrlimit(Resource::RLIMIT_NOFILE).expect("get trlimit failed!");
    let result = unsafe { fork().expect("cant't fork a new process") };
    if result.is_parent() {
        unsafe { exit(0) };
    }
    unsafe {
        signal(Signal::SIGPIPE, SigHandler::SigIgn).unwrap();
        signal(Signal::SIGHUP, SigHandler::SigIgn).unwrap();
    }
    pthread_sigmask(SigmaskHow::SIG_BLOCK, Some(&SigSet::all()), None).unwrap();
    setsid().expect("can't set sid");
    chdir("/").unwrap();
}

pub fn already_running() -> bool {
    let lock_mode = Mode::S_IRUSR | Mode::S_IWUSR | Mode::S_IRGRP | Mode::S_IROTH;
    let fd = open(LOCK_FILE, OFlag::O_RDWR | OFlag::O_CREAT, lock_mode).unwrap();
    match flock(fd, FlockArg::LockExclusiveNonblock) {
        Ok(_) => (),
        Err(_) => return false,
    }
    match ftruncate(fd, 0) {
        Ok(_) => (),
        Err(_) => return false,
    }
    let pid = getpid();
    let buf = format!("{}", pid);
    match write(fd, buf.as_bytes()) {
        Ok(0) | Err(_) => return true,
        _ => return false,
    }
}
