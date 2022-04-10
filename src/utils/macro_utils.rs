#[macro_export]
macro_rules! bits {
    ($expression:expr) => {
        use nix::sys::epoll::EpollFlags;
        ($expression).bits() > 0
    };
}

#[macro_export]
macro_rules! is_reg {
    ($expression:expr) => {
        ($expression) & SFlag::S_IFREG.bits() == SFlag::S_IFREG.bits()
    };
}

#[macro_export]
macro_rules! is_link {
    ($expression:expr) => {
        ($expression) & SFlag::S_IFLNK.bits() == SFlag::S_IFLNK.bits()
    };
}

#[macro_export]
macro_rules! is_dir {
    ($expression:expr) => {
        ($expression) & SFlag::S_IFDIR.bits() == SFlag::S_IFDIR.bits()
    };
}

#[macro_export]
macro_rules! is_sock {
    ($expression:expr) => {
        ($expression) & SFlag::S_IFSOCK.bits() == SFlag::S_IFSOCK.bits()
    };
}

#[macro_export]
macro_rules! is_char {
    ($expression:expr) => {
        ($expression) & SFlag::S_IFCHR.bits() == SFlag::S_IFCHR.bits()
    };
}

#[macro_export]
macro_rules! is_blk {
    ($expression:expr) => {
        ($expression) & SFlag::S_IFBLK.bits() == SFlag::S_IFBLK.bits()
    };
}

#[macro_export]
macro_rules! is_pipe {
    ($expression:expr) => {
        ($expression) & SFlag::S_IFIFO.bits() == SFlag::S_IFIFO.bits()
    };
}
