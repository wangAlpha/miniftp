use super::cmd::*;
use super::codec::{Encoder, FtpCodec};
use crate::server::connection::{ConnRef, Connection, State};
use crate::server::server::EventLoop;
use log::{debug, info, warn};
use nix::fcntl::OFlag;
use nix::sys::socket::{accept4, SockFlag};
use nix::unistd::{close, lseek, write};
use nix::{fcntl::open, sys::stat::Mode};
use std::collections::HashMap;
use std::net::TcpListener;
use std::os::unix::prelude::AsRawFd;
use std::path::{Component, Path, PathBuf, StripPrefixError};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
enum DataType {
    ASCII,
    BINARY,
}

#[derive(Debug, Clone)]
pub struct Context(pub(crate) i32, pub(crate) usize);

impl Context {
    fn new(fd: i32, offset: usize) -> Context {
        Context { 0: fd, 1: offset }
    }
}

#[derive(Debug, Clone)]
pub struct Session {
    cwd: PathBuf,
    cmd_conn: ConnRef,
    data_conn: Option<ConnRef>,
    data_port: Option<u16>,
    // name: Option<String>,
    codec: FtpCodec,
    server_root: PathBuf,
    is_admin: bool,
    transfer_type: TransferType,
    curr_file_context: Option<Context>,
    waiting_pwd: bool,
    event_loop: EventLoop,
}

impl Session {
    pub fn new(conn: ConnRef, event_loop: EventLoop) -> Self {
        Session {
            cwd: PathBuf::new(),
            cmd_conn: conn,
            data_conn: None,
            data_port: Some(2222),
            codec: FtpCodec,
            // config: Config,
            server_root: PathBuf::new(),
            is_admin: true,
            transfer_type: TransferType::ASCII,
            waiting_pwd: false,
            event_loop,
            curr_file_context: None,
        }
    }
    pub fn handle_command(&mut self, msg: Vec<u8>, conn_map: &mut Arc<Mutex<HashMap<i32, i32>>>) {
        let cmd = Command::new(msg).unwrap();
        let connected = self.cmd_conn.lock().unwrap().connected();
        if connected && self.is_logged() {
            match cmd {
                Command::User(_) => self.send_answer(Answer::new(ResultCode::Ok, "Login success")),
                Command::Pasv => self.pasv(conn_map),
                Command::Port(port) => {
                    self.data_port = Some(port);
                    self.send_answer(Answer::new(
                        ResultCode::Ok,
                        format!(" Data port is now {}", port).as_str(),
                    ));
                    // Wait for ready reply
                    let addr = format!("127.0.0.1:{}", port);
                    let mut c = Connection::connect(addr.as_str());

                    let cmd_fd = self.cmd_conn.lock().unwrap().get_fd();
                    let data_fd = c.get_fd();
                    c.register_read(&mut self.event_loop);
                    self.data_conn = Some(Arc::new(Mutex::new(c)));
                    conn_map.lock().unwrap().insert(cmd_fd, data_fd);
                }
                Command::Type(typ) => {
                    self.transfer_type = typ;
                    self.cmd_conn
                        .lock()
                        .unwrap()
                        .send(&format!("OK Type: {:?}", typ).as_bytes());
                }
                Command::Stor(path) => {
                    self.stor(path);
                }
                _ => (),
            }
        } else {
            debug!("cmd_conn: {} is logged: {}", connected, self.is_logged());
            // PASS
        }
        // match cmd {
        //     Command::Auth => println!("Auth"),
        //     Command::Quit => println!("Quit"),
        //     Command::NoOp => println!("NoOp"),
        //     Command::Pasv => println!(),
        //     Command::Syst => println!(),
        //     Command::Type(typ) => {
        //         self.transfer_type = typ;
        //         self.send_answer(Answer::new(
        //             ResultCode::Ok,
        //             "Transfer type changed successfully",
        //         ));
        //     }
        //     Command::User(content) => println!("TODO User"),
        //     Command::Unknown(s) => {
        //         self.send_answer(Answer::new(
        //             ResultCode::SyntaxErr,
        //             &format!("\"{}\": Not implemented", s),
        //         ));
        //     }
        // }
    }
    fn is_logged(&self) -> bool {
        true
    }
    fn deregister_conn(&mut self) {}
    fn complete_path(&self, path: PathBuf) {}
    fn mkd(&mut self, path: PathBuf) {}
    fn rmd(&mut self, dir: PathBuf) {}

    fn cwd(&mut self, path: Option<PathBuf>) {}
    fn list(&mut self, path: Option<PathBuf>) {}
    fn pasv(&mut self, conn_map: &mut Arc<Mutex<HashMap<i32, i32>>>) {
        let port = if let Some(port) = self.data_port {
            port
        } else {
            // let mut addr = self.cmd_conn.lock().unwrap().get_peer_address();
            22
        };
        if self.data_conn.is_some() {
            self.send_answer(Answer::new(
                ResultCode::DataConnOpened,
                "Already listening...",
            ));
            return;
        }
        let addr = format!("0.0.0.0:{}", port);
        let listener = TcpListener::bind(addr).unwrap();
        self.send_answer(Answer::new(ResultCode::Ok, "PASV is start"));
        let mut c = Connection::accept(listener.as_raw_fd());
        c.register_read(&mut self.event_loop);
        self.data_conn = Some(Arc::new(Mutex::new(c)));
        let cmd_fd = self.cmd_conn.lock().unwrap().get_fd();
        let data_fd = self.data_conn.clone().unwrap().lock().unwrap().get_fd();
        conn_map.lock().unwrap().insert(data_fd, cmd_fd);
    }
    fn quit(&mut self) {}
    fn retr(&mut self, path: PathBuf) {}

    fn stor(&mut self, path: PathBuf) {
        if self.data_conn.is_some() {
            // check file path and admin
            let path = self.cwd.join(path);
            if !invaild_path(&path) && self.is_admin {
                self.send_answer(Answer::new(
                    ResultCode::DataConnOpened,
                    "Starting to send file...",
                ));
                let path = path.as_os_str();
                let oflag: OFlag = OFlag::O_CREAT | OFlag::O_RDWR;
                let file = open(path, oflag, Mode::all()).unwrap();
                self.curr_file_context = Some(Context::new(file, 0));
            }
        } else {
            self.send_answer(Answer::new(
                ResultCode::DataConnFail,
                "No opened data connection",
            ));
        }
    }

    pub fn receive_data(&mut self, msg: Vec<u8>, conn: &Arc<Mutex<Connection>>) {
        debug!(
            "receive_data: {}, conn: {}",
            msg.len(),
            conn.lock().unwrap().get_fd()
        );
        if let Some(context) = &self.curr_file_context {
            let buf = msg.as_slice();
            write(context.0, buf).unwrap();
            let s = conn.lock().unwrap().get_state();
            debug!("After receive data conn state: {:?}", s);
            if s != State::Finished && s != State::Closed {
                close(context.0).unwrap();
                self.curr_file_context = None;
                self.send_answer(Answer::new(ResultCode::ConnClose, "Transfer done"));
                info!("Transfer done!");
            }
        } else {
            warn!("cant't get current file context");
        }
    }

    fn send_answer(&mut self, answer: Answer) {
        self.cmd_conn
            .lock()
            .unwrap()
            .send(format!("{} {}", answer.code as i32, answer.message).as_bytes());
    }
}
// use  std::path::{Component, Path, };

fn invaild_path(path: &Path) -> bool {
    for component in path.components() {
        if let Component::ParentDir = component {
            return true;
        }
    }
    false
}

fn strip_prefix(dir: PathBuf) {}
