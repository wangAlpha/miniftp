use nix::sys::socket::{accept4, SockFlag};

use super::cmd::*;
use super::codec::{Encoder, FtpCodec};
use crate::server::connection::ConnRef;
use crate::server::connection::Connection;
use crate::server::server::EventLoop;
use std::collections::HashMap;
use std::net::TcpListener;
use std::os::unix::prelude::AsRawFd;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

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
    data_conn: Option<Arc<Connection>>,
    data_port: Option<u16>,
    // name: Option<String>,
    codec: FtpCodec,
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
            cmd_conn: conn,
            data_conn: None,
            data_port: Some(2222),
            codec: FtpCodec,
            // config: Config,
            server_root: PathBuf::new(),
            is_admin: false,
            transfer_type: TransferType::ASCII,
            waiting_pwd: false,
            event_loop,
        }
    }
    pub fn handle_command(&mut self, msg: Vec<u8>, conn_map: &mut Arc<Mutex<HashMap<i32, i32>>>) {
        println!(
            "handle command: {}",
            String::from_utf8(msg.clone()).unwrap()
        );
        let cmd = Command::new(msg).unwrap();
        if self.is_logged() {
            match cmd {
                Command::Pasv => self.pasv(conn_map),
                Command::Port(port) => {
                    self.data_port = Some(port);
                    self.cmd_conn
                        .lock()
                        .unwrap()
                        .send(&format!("OK Data port is now {}", port).as_bytes());
                    let addr = format!("127.0.0.1:{}", port);
                    let mut c = Connection::connect(addr.as_str());

                    let cmd_fd = self.cmd_conn.lock().unwrap().get_fd();
                    let data_fd = c.get_fd();
                    c.register_read(&mut self.event_loop);
                    self.data_conn = Some(Arc::new(c));
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
    fn strip_prefix(&self, dir: PathBuf) {}
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
        let mut c = Connection::accept(listener.as_raw_fd());
        c.register_read(&mut self.event_loop);
        self.data_conn = Some(Arc::new(c));
        let cmd_fd = self.cmd_conn.lock().unwrap().get_fd();
        let data_fd = self.data_conn.as_ref().unwrap().get_fd();
        conn_map.lock().unwrap().insert(cmd_fd, data_fd);
    }
    fn quit(&mut self) {}
    fn retr(&mut self, path: PathBuf) {}
    fn stor(&mut self, path: PathBuf) {
        if let Some(conn) = &self.data_conn {
            // check file path and admin
            let path = self.cwd.join(path);
        } else {
            self.send_answer(Answer::new(
                ResultCode::DataConnFail,
                "No opened data connection",
            ));
        }
    }
    pub fn receive_data(&mut self, msg: Vec<u8>, conn: Connection) {
        if self.data_conn.is_none() {
            self.data_conn = Some(Arc::new(conn));
        }
        println!("receive data");
    }
    // TODO code
    fn send_answer(&mut self, answer: Answer) {
        self.cmd_conn
            .lock()
            .unwrap()
            .send(format!("{:?}", answer).as_bytes());
    }
}
