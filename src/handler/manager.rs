use crate::connection::Connection;
use nix::sys::socket::SockAddr;
use std::path::Path;
use std::sys::home_dir;

enum Mode {
    PASV,
    PORT,
}
pub struct Manager {
    address: SockAddr,
    cur_path: Path,
    data_conn: ConnRef,
    cmd_conn: ConnRef,
    mode: Mode,
}
impl Manager {
    pub fn new() -> Self {
        Manager {}
    }
}
