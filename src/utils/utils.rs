use chrono::Local;
use env_logger::Builder;
use log::LevelFilter;
use nix::fcntl::{flock, open, FlockArg, OFlag};
use nix::libc::{STDERR_FILENO, STDOUT_FILENO};
use nix::sys::signal::{pthread_sigmask, signal};
use nix::sys::signal::{SigHandler, SigSet, SigmaskHow, Signal};
use nix::sys::stat::{lstat, umask, Mode, SFlag};
use nix::unistd::{access, chdir, dup2, fork};
use nix::unistd::{ftruncate, getpid, getuid, write};
use nix::unistd::{AccessFlags, Uid, User};
use std::io::Write;

const LOCK_FILE: &'static str = "/var/run/miniftp.pid";
const LOG_FILE: &'static str = "/var/log/mini.log";

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

pub fn is_exist(path: &str) -> bool {
    access(path, AccessFlags::F_OK).is_ok()
}

pub fn daemonize() {
    umask(Mode::from_bits(0x00).unwrap());
    let log_fd = open(
        LOG_FILE,
        OFlag::O_APPEND | OFlag::O_NONBLOCK | OFlag::O_CLOEXEC,
        Mode::S_IWUSR | Mode::S_IRUSR,
    )
    .unwrap();
    let result = unsafe { fork().expect("cant't fork a new process") };

    dup2(log_fd, STDERR_FILENO).unwrap();
    dup2(log_fd, STDOUT_FILENO).unwrap();

    unsafe {
        signal(Signal::SIGPIPE, SigHandler::SigIgn).unwrap();
        signal(Signal::SIGHUP, SigHandler::SigIgn).unwrap();
    }
    pthread_sigmask(SigmaskHow::SIG_BLOCK, Some(&SigSet::all()), None).unwrap();
    // setsid().expect("can't set sid");
    let root = User::from_uid(Uid::from_raw(0)).unwrap().unwrap();
    chdir(&root.dir).expect("Couldn't cd to root directory");
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

pub fn is_root_user() -> bool {
    getuid().is_root()
}

pub fn set_log_level(level: LevelFilter) {
    let mut builder = Builder::new();
    builder.format(|buf, record| {
        writeln!(
            buf,
            "{} {} {}:{} - {}",
            Local::now().format("%m-%d %H:%M:%S"),
            record.level(),
            record.file().unwrap(),
            record.line().unwrap(),
            record.args(),
        )
    });
    builder.parse_env("RUST_LOG");
    builder.filter(None, level).init();
}
