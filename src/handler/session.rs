use super::cmd::*;
use crate::server::{connection::ConnRef, server::EventLoop};
use crate::utils::config::Config;
use nix::sys::socket::SockAddr;
use std::{path::PathBuf, str::FromStr, sync::Arc};

#[derive(Debug, Clone)]
enum Mode {
    PASV,
    PORT,
}

#[derive(Debug, Clone)]
enum DataType {
    ASCII,
    BINARY,
}

#[derive(Debug, Clone)]
pub struct Session {
    cwd: PathBuf,
    cmd_conn: ConnRef,
    data_conn: Option<ConnRef>,
    // data_port: Option<u16>,
    // data_conn: Option<ConnRef>,
    // name: Option<String>,
    server_root: PathBuf,
    is_admin: bool,
    transfer_type: TransferType,
    waiting_pwd: bool,
    event_loop: EventLoop,
}

impl Session {
    pub fn new(conn: ConnRef, event_loop: EventLoop) -> Self {
        Session {
            cwd: PathBuf::new(),
            // data_port: None,
            data_conn: None,
            cmd_conn: conn,
            server_root: PathBuf::new(),
            // config: Config,
            is_admin: false,
            transfer_type: TransferType::ASCII,
            waiting_pwd: false,
            event_loop,
        }
    }
    // pub fn get_context(&self, conn: ConnRef) -> Rc<Context> {
    //     let fd = conn.lock().unwrap().get_fd();
    //     self.contexts[&fd]
    // }
    pub fn handle_command(&self, msg: Vec<u8>) -> Result<String, Error> {
        let cmd = Command::new(msg).ok()?;

        println!("session recevice a msg: {:?}", cmd);
        if self.is_logged() {
            match cmd {
                Command::Pasv => self.pasv(),
                Command::Port(port) => {
                    Some(port)
                }
            }
        }
        // match cmd {
        //     // b"USER" => self.user(),
        //     Command::Type() => self.user(),
        // }
        // Access control commands
        //"",
        //"PASS",
        //"ACCT",
        //"CWD",
        //"CDUP",
        //"REIN",
        //"QUIT",
        // Transfer parameter commands
        //"PORT",
        //"PASV",
        //"TYPE",
        //"MODE",
        // Ftp service commands
        //"RETR",
        //"STOR",
        //"STOU",
        //"APPE",
        //"ALLO",
        //"REST",
        //"RNFR",
        //"RNTO",
        //"ABOR",
        //"DELE",
        //"RMD",
        //"MKD",
        //"PWD",
        //"LIST",
        //"NLST",
        //"SITE",
        //"SYST",
        //"STAT",
        //"HELP",
        //"NOOP",
        // Modern FTP Commands
        //"FEAT",
        //"OPTS",
        //"SIZE",
        Ok(String::from_str("aaaaa"))
    }
    fn is_logged(&self) -> bool {
        true
    }
    fn port(&self) -> bool {
        false
    }
    fn deregister_conn(&mut self) {}
    fn complete_path(&self, path: PathBuf) {}
    fn mkd(&mut self, path: PathBuf) {}
    fn rmd(&mut self, dir: PathBuf) {}
    fn strip_prefix(&self, dir: PathBuf) {}
    fn cwd(&mut self, path: Option<PathBuf>) {}
    fn list(&mut self, path: Option<PathBuf>) {}
    fn pasv(&mut self) {
        // let (fd, listener) = Connection::bind("0.0.0.0:8090");

        // event_loop_clone.register(
        //     listener,
        //     EpollFlags::EPOLLHUP
        //         | EpollFlags::EPOLLERR
        //         | EpollFlags::EPOLLIN
        //         | EpollFlags::EPOLLOUT
        //         | EpollFlags::EPOLLET,
        // );
    }
    fn quit(&mut self) {}
    fn retr(&mut self, path: PathBuf) {}
    fn stor(&mut self, path: PathBuf) {}
    fn send(&mut self) {}
    // fn receive_data(&mut self) {}
}

fn file_info(path: PathBuf, out: &mut Vec<u8>) {}
