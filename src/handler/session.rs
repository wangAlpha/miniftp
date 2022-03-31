use super::cmd::*;
use super::codec::{Encoder, FtpCodec};
use crate::server::connection::ConnRef;
use crate::server::connection::Connection;
use crate::server::server::EventLoop;
use std::{path::PathBuf, sync::Arc};

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
            data_port: Some(8090),
            codec: FtpCodec,
            // config: Config,
            server_root: PathBuf::new(),
            is_admin: false,
            transfer_type: TransferType::ASCII,
            waiting_pwd: false,
            event_loop,
        }
    }
    pub fn handle_command(&mut self, msg: Vec<u8>) {
        let cmd = Command::new(msg).ok().unwrap();
        println!("session recevice a msg: {:?}", cmd);
        if self.is_logged() {
            match cmd {
                Command::Pasv => self.pasv(),
                Command::Port(port) => {
                    self.cmd_conn
                        .lock()
                        .unwrap()
                        .send(&format!("OK Data port is now {}", port).as_bytes());
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
        let port = if let Some(port) = self.data_port {
            port
        } else {
            // let mut addr = self.cmd_conn.lock().unwrap().get_peer_address();
            22
        };
        let (fd, listener) = Connection::bind(format!("0.0.0.0:{}", port).as_str());
        self.event_loop.register_listen(listener);
        println!("register pasv port: {}", port);
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
    // TODO
    fn send_answer(&mut self, answer: Answer) {
        self.cmd_conn
            .lock()
            .unwrap()
            .send(format!("{:?}", answer).as_bytes());
        // let mut bytes = BytesMut::new();
        // self.codec.encode(answer, &mut bytes);
        // let a = "".as_bytes();
        // answer.as_bytes()
        // bytes.
    }
    // fn receive_data(&mut self) {}
}

fn file_info(path: PathBuf, out: &mut Vec<u8>) {}
