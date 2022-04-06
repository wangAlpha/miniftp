macro_rules! bits {
    ($expression:expr) => {
        use nix::sys::epoll::EpollFlags;
        ($expression).bits() > 0
    };
}
