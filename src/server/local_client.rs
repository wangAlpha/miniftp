use super::connection::Connection;
use crate::handler::cmd::{Answer, ResultCode};
use crate::server::record_lock::{self, FileLock};
use log::{debug, info, warn};
use nix::fcntl::{self, open, OFlag};
use nix::sys::sendfile::sendfile;
use nix::sys::socket::SockFlag;
use nix::sys::socket::{self, accept4, setsockopt, sockopt};
use nix::sys::stat::lstat;
use nix::sys::stat::{Mode, SFlag};
use std::net::TcpListener;
use std::os::unix::prelude::AsRawFd;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::{
    io::{self, stdin, Write},
    ops::BitAnd,
};

#[derive(Debug)]
pub struct LocalClient {
    hostname: String,
    port: u16,
    connected: bool,
    quit: bool,
    cmd_conn: Option<Connection>,
    data_conn: Option<Connection>,
}

impl LocalClient {
    pub fn new() -> Self {
        LocalClient {
            hostname: String::new(),
            port: 8089,
            connected: false,
            quit: false,
            cmd_conn: None,
            data_conn: None,
        }
    }
    pub fn shell_loop(&mut self) {
        // let config = Config::from_cwd_config();
        let mut line = String::new();

        loop {
            print!("FTP> ");
            io::stdout().flush().unwrap();
            match stdin().read_line(&mut line) {
                Ok(n) => {
                    line.pop();
                    if n == 0 {
                        continue;
                    }
                    self.handle_cmd(&mut line);
                    line.clear();
                }
                Err(error) => println!("error: {}", error),
            }
            if self.quit {
                break;
            }
        }
    }
    pub fn handle_cmd(&mut self, line: &mut String) -> String {
        // 去回车符号
        let mut commands = Vec::new();
        for s in line.split_ascii_whitespace() {
            commands.push(String::from_str(s).unwrap());
        }
        if commands.is_empty() {
            return String::from("Invaild command");
        }
        let (cmd, args) = (commands[0].to_uppercase(), &commands[1..]);
        match cmd.as_bytes() {
            b"OPEN" => self.open(args),
            b"USER" => self.user(),
            b"PASV" => {
                let s = self.send_cmd(String::from_str("PASV").unwrap());
                info!("Reply msg: {}", s);
                let port = 2222u16;
                let addr = format!("127.0.0.1:{}", port);
                self.data_conn = Some(Connection::connect(addr.as_str()));
            }
            b"PORT" => self.port(),
            b"CLOSE" => self.close(),
            b"CD" => self.cd(),
            // b"LS" => self.list(),
            b"PUT" => {
                for path in args {
                    let path = Path::new(path);
                    self.stor(path);
                }
            }
            b"GET" => self.get(),
            // b"PWD" => self.pwd(),
            // b"MKDIR" => self.mkdir(),
            // b"RMDIR" => self.rmdir(),
            b"DEL" => self.del(),
            b"STAT" => self.stat(),
            b"SYST" => self.syst(),
            b"BINARY" => self.binary(),
            b"SIZE" => self.size(),
            b"NOOP" => self.noop(),
            b"HELP" => self.help(),
            b"EXIT" | b"QUIT" => self.exit(),
            _ => println!("?Invalid command"),
        };
        String::from("_")
    }
    fn open(&mut self, args: &[String]) {
        // println!("args: {:?}", args);
        // if self.is_open() {
        //     println!("Already connected to localhost, use close first.");
        //     return;
        // }
        // if args.is_empty() {
        //     print!("(to)");
        //     io::stdout().flush().unwrap();
        //     stdin().read_line(&mut self.hostname).expect("input ");
        // } else {
        //     self.hostname = args[0].clone();
        // }
        self.user();
    }
    fn user(&mut self) {
        // let mut username = String::new();
        // let mut password = String::new();
        // print!("Name ({}:user):", self.hostname);
        // stdin().read_line(&mut username).unwrap();
        // print!("Password:");
        // stdin().read_line(&mut password).unwrap();

        let username = String::new();
        let password = String::new();
        let addr = format!("127.0.0.1:8089");
        self.cmd_conn = Some(Connection::connect(addr.as_str()));
        self.login(username, password);
        self.binary();
    }
    fn login(&mut self, username: String, password: String) {
        if self.is_open() {}
        let reply = self.send_cmd(format!("USER {}", username));
        println!("reply: {}", reply);
        // if reply.code == ResultCode::NeedPsw {
        //     println!("{:?}", self.send_cmd(format!("PASS {}", password)));
        // } else {
        // }
    }
    fn close(&mut self) {}
    fn is_open(&self) -> bool {
        false
    }
    fn send_cmd(&mut self, cmd: String) -> String {
        debug!("send msg: {}", cmd);
        self.cmd_conn.as_mut().unwrap().send(cmd.as_bytes());
        self.cmd_conn.as_mut().unwrap().read();
        let buf = self.cmd_conn.as_mut().unwrap().get_msg();
        String::from_utf8_lossy(&buf).to_string()
    }
    fn port(&mut self) {
        // TODO: check status code
        if let Some(c) = self.data_conn.clone() {
            c.shutdown();
            self.data_conn = None;
        }
        // TODO file configure
        let port = 2222u16;
        let cmd = format!("PORT 127,0,0,1,{},{}", port >> 8, 0xFF & port);
        self.send_cmd(cmd);
        let addr = format!("{}:{}", "127.0.0.1", port);
        let listener = TcpListener::bind(addr.as_str()).unwrap();
        let fd = accept4(listener.as_raw_fd(), SockFlag::SOCK_CLOEXEC).unwrap();
        setsockopt(fd, sockopt::TcpNoDelay, &true).unwrap();
        self.data_conn = Some(Connection::new(fd));
        println!("data connection build success");
    }

    fn cd(&mut self) {}
    fn stor(&mut self, path: &Path) {
        // TODO: check file position.
        // check connection
        // 上传文件的默认权限
        // 支持断点续传
        // 在服务端进行限速
        self.send_cmd(format!("STOR {}", path.display()));
        if let Some(c) = self.data_conn.clone() {
            self.send_answer(Answer::new(
                ResultCode::DataConnOpened,
                "Starting to send file...",
            ));
            let fd = open(path, OFlag::O_RDONLY, Mode::all()).unwrap();
            let lock = FileLock::new(fd);
            lock.lock(false);

            if is_regular(path) {
                let len = get_file_size(path);
                let len = sendfile(c.get_fd(), fd, None, len).unwrap();
                debug!("-> file: {:?} transfer done! size: {}", path, len);
            } else {
                warn!("{:?} is not regular file", path);
            }
        } else {
            warn!("No opened data connection!");
        }
    }
    fn send_answer(&mut self, answer: Answer) {
        if let Some(c) = self.cmd_conn.as_mut() {
            let buf = format!("{} {}", answer.code as i32, answer.message);
            c.send(buf.as_bytes());
        }
    }
    fn invaild_path(&self, path: &str) -> bool {
        // TODO
        true
    }
    fn upload(&mut self) {}
    fn get(&mut self) {}
    fn del(&mut self) {}
    fn binary(&mut self) {
        self.send_cmd(String::from("TYPE I"));
    }
    fn size(&mut self) {}
    fn stat(&mut self) {}
    fn syst(&mut self) {}
    fn noop(&mut self) {}
    fn help(&mut self) {
        print!(
            "List of ftp commands:\n
      open hostname [ port ] - open new connection\n
      user username - send new user information\n
      cd remote-directory - change remote working directory\n
      ls [ remote-directory ] - print list of files in the remote directory\n
      put local-file [ remote-file ] - store a file at the server\n
      get remote-file [ local-file ] - retrieve a copy of the file\n
      pwd - print the current working directory name\n
      mkdir directory-name - make a directory on the remote machine\n
      rmdir directory-name - remove a directory\n
      del remote-file - delete a file\n
      binary - set binary transfer type\n
      size remote-file - show size of remote file\n
      stat [ remote-file ] - print server information\n
      syst - show remote system type\n
      noop - no operation\n
      close - close current connection\n
      help - print list of ftp commands\n
      exit - exit program\n"
        );
    }
    fn exit(&mut self) {
        if self.is_open() {
            self.cmd_conn.as_mut().unwrap().shutdown();
            // self.data_conn.clone().unwrap().lock().shutdown();
            self.cmd_conn = None;
            self.data_conn = None;
        }
        self.quit = true;
    }
}

pub fn is_regular(path: &Path) -> bool {
    match lstat(path) {
        Ok(stat) => SFlag::S_IFREG.bits() & stat.st_mode == SFlag::S_IFREG.bits(),
        Err(e) => {
            warn!("Can't get path {:?} state, {}", path, e);
            false
        }
    }
}

pub fn get_file_size(path: &Path) -> usize {
    match lstat(path) {
        Ok(stat) => stat.st_size as usize,
        Err(e) => {
            warn!("Can't get path {:?} state, {}", path, e);
            0
        }
    }
}
