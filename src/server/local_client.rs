use crate::handler::cmd::{Answer, ResultCode};
use crate::handler::codec::{BytesCodec, Decoder, Encoder};
use crate::net::connection::Connection;
use crate::server::record_lock::FileLock;
use crate::utils::utils::is_regular;
use log::{debug, info, warn};
use nix::fcntl::{open, OFlag};
use nix::sys::sendfile::sendfile;
use nix::sys::socket::{accept4, setsockopt, sockopt, SockFlag};
use nix::sys::stat::{lstat, Mode};
use std::io::{self, stdin, Write};
use std::net::TcpListener;
use std::os::unix::prelude::AsRawFd;
use std::path::Path;
use std::str::FromStr;
use std::time::Instant;

#[derive(Debug)]
pub struct LocalClient {
    hostname: String,
    port: u16,
    connected: bool,
    quit: bool,
    cmd_conn: Option<Connection>,
    data_conn: Option<Connection>,
    codec: BytesCodec,
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
            codec: BytesCodec,
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
        let args = args.to_vec();
        match cmd.as_bytes() {
            b"OPEN" => self.open(&args),
            b"USER" => self.user(),
            b"PASV" => {
                let s = self.send_cmd("PASV").unwrap();
                let port = self.port + 1;
                let addr = format!("127.0.0.1:{}", port);
                self.data_conn = Some(Connection::connect(addr.as_str()));
            }
            b"ABOR" => self.abort(),
            b"CLOSE" => self.close(),
            b"CD" => self.cd(&args[0]),
            b"LS" => self.list(&args),
            b"PUT" => self.put(&args),
            b"GET" => self.get(&args),
            b"PWD" => self.pwd(),
            // b"MKDIR" => self.mkdir(),
            // b"RMDIR" => self.rmdir(),
            b"DEL" => self.delete(&args),
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
    fn open(&mut self, args: &Vec<String>) {
        println!("args: {:?}", args);
        if self.is_open() {
            println!("Already connected to localhost, use close first.");
            return;
        }
        if args.is_empty() {
            print!("(to) ");
            io::stdout().flush().unwrap();
            stdin().read_line(&mut self.hostname).expect("input ");
        } else {
            self.hostname = args[0].clone();
            if args.len() >= 2 {
                match args[1].parse::<u16>() {
                    Ok(port) => self.port = port,
                    Err(_) => {
                        info!("usage: open host-name [port]");
                        return;
                    }
                }
            }
        }
        self.user();
    }
    fn user(&mut self) {
        // print!("Name ({}:user):", self.hostname);
        // stdin().read_line(&mut username).unwrap();
        // print!("Password:");
        // stdin().read_line(&mut password).unwrap();

        let username = String::new();
        let password = String::new();
        let port = 8089;
        let addr = format!("127.0.0.1:{}", port);
        debug!("Connect ftp server: {}", addr);
        self.cmd_conn = Some(Connection::connect(&addr));
        self.login(username, password);
        self.binary();
    }
    fn login(&mut self, username: String, password: String) {
        if !self.is_open() {
            warn!("Not connected.");
        }
        let reply = self.send_cmd(&format!("USER {}", username)).unwrap();
        if reply.code == ResultCode::NeedPsw {
            let msg = format!("PASS {}", password);
            self.send_cmd(&msg).unwrap();
        }
    }
    fn close(&mut self) {
        self.send_cmd("CLOSE");
        self.cmd_conn = None;
        self.data_conn = None;
    }
    fn is_open(&self) -> bool {
        self.cmd_conn.is_some()
    }
    fn pwd(&mut self) {
        self.send_cmd("PWD");
    }
    fn list(&mut self, args: &[String]) {
        // Example:
        // 200 PORT command successful. Consider using PASV.
        // 150 Here comes the directory listing.
        // -rwxr-xr-x    1 0        0        21863760 Apr 03 20:09 miniftp
        // 226 Directory send OK.
        self.port();
        let mut cmd = "LIST".to_string();
        for s in args.iter() {
            cmd.push(' ');
            cmd.push_str(s)
        }
        self.send_cmd(&cmd);
        let msg = self.receive_data();
        println!("{}", String::from_utf8(msg).unwrap());
    }
    fn receive_data(&mut self) -> Vec<u8> {
        if let Some(ref mut c) = self.data_conn {
            // ugly function
            c.read();
            let msg = c.get_msg();
            return msg;
        }
        Vec::new()
    }

    fn send_file(&mut self, file: &str) -> usize {
        if let Some(ref mut c) = self.data_conn {
            return c.send_file(file).unwrap_or(0);
        }
        0
    }

    fn send_cmd(&mut self, cmd: &str) -> Option<Answer> {
        debug!("send msg: {}", cmd);
        let buf = cmd.as_bytes().to_vec();
        let mut msg = Vec::new();
        self.codec.encode(buf, &mut msg).unwrap();
        self.cmd_conn.as_mut().unwrap().send(&msg);
        // FIXME: 这个read貌似有bug
        self.cmd_conn.as_mut().unwrap().read();
        let mut msg = self.cmd_conn.as_mut().unwrap().get_msg();

        let answer = self.codec.decode(&mut msg).unwrap();
        if let Some(answer) = answer {
            Some(answer)
        } else {
            None
        }
    }

    fn port(&mut self) {
        // TODO: check status code
        if let Some(c) = self.data_conn.clone() {
            c.shutdown();
            self.data_conn = None;
        }
        // FIXME: a connection bug
        // get current local connection {port}
        let port = self.port + 1;
        let cmd = format!("PORT 127,0,0,1,{},{}", port >> 8, 0xFF & port);
        self.send_cmd(&cmd);
        let addr = format!("{}:{}", "127.0.0.1", port);
        let listener = TcpListener::bind(addr.as_str()).unwrap();
        debug!("listener: {:?}", listener);
        let fd = accept4(listener.as_raw_fd(), SockFlag::SOCK_CLOEXEC).unwrap();
        debug!("accept a new connection: {}", fd);
        setsockopt(fd, sockopt::TcpNoDelay, &true).unwrap();
        self.data_conn = Some(Connection::new(fd));
        debug!("data connection build success");
    }

    fn cd(&mut self, path: &String) {
        let msg = format!("CD {}", path);
        self.send_cmd(&msg);
    }

    fn stor(&mut self, path: &Path) {
        // TODO: check file position.
        // check connection
        // 上传文件的默认权限
        // 支持断点续传
        // 在服务端进行限速
        self.send_cmd(&format!("STOR {:?}", path.display()));
        if let Some(c) = self.data_conn.clone() {
            self.send_answer(Answer::new(
                ResultCode::DataConnOpened,
                "Starting to send file...",
            ));
            let fd = open(path, OFlag::O_RDONLY, Mode::all()).unwrap();
            let lock = FileLock::new(fd);
            lock.lock(false);

            if is_regular(path.to_str().unwrap()) {
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
    fn get(&mut self, files: &Vec<String>) {
        // local: miniftp remote: miniftp
        // 200 PORT command successful. Consider using PASV.
        // 150 Opening BINARY mode data connection for miniftp (21863760 bytes).
        // 226 Transfer complete.
        // 21863760 bytes received in 10.59 secs (1.9698 MB/s)

        let mut total_size = 0usize;
        let start = Instant::now();
        if let Some(ref c) = self.data_conn {
            for file in files.iter() {
                let answer = self.send_cmd(&format!("RETR {}", file)).unwrap();
                // if answer.code =
                // 用正则表
                let buf = self.receive_data();
                total_size += buf.len();
            }
        }
        let duration = start.elapsed();
        let rate = total_size as f64 / 1024f64 / 1024f64 / duration.as_secs_f64();
        self.receive_answer();
        println!(
            "{} bytes received in {} secs ({} MB/s)",
            total_size,
            duration.as_secs_f32(),
            rate
        );
    }
    fn receive_answer(&self) {}
    fn put(&mut self, files: &Vec<String>) {
        // local: miniftp remote: miniftp
        // 200 PORT command successful. Consider using PASV.
        // 150 Opening BINARY mode data connection for miniftp (21863760 bytes).
        // 226 Transfer complete.
        // 21863760 bytes received in 10.59 secs (1.9698 MB/s)

        self.port();
        self.binary();
        let mut total_size = 0usize;
        let start = Instant::now();
        // TODO: send file
        if self.data_conn.is_some() {
            for file in files.iter() {
                if let Some(answer) = self.send_cmd(&format!("STOR {}", file)) {
                    if answer.code != ResultCode::DataConnOpened && answer.code != ResultCode::Ok {
                        total_size += self.send_file(file);
                    }
                }
            }
        }
        let duration = start.elapsed();
        let rate = total_size as f64 / 1024f64 / 1024f64 / duration.as_secs_f64();
        println!(
            "{} bytes received in {} secs ({} MB/s)",
            total_size,
            duration.as_secs_f32(),
            rate
        );
    }
    fn delete(&mut self, files: &Vec<String>) {
        files.iter().for_each(|f| {
            self.send_cmd(&format!("DELE {}", f));
        });
    }
    fn binary(&mut self) {
        self.send_cmd("TYPE I");
    }
    fn size(&mut self) {}
    fn stat(&mut self) {}
    fn syst(&mut self) {
        self.send_cmd("SYST");
    }
    fn noop(&mut self) {
        self.send_cmd("NOOP");
    }
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
    fn abort(&mut self) {
        self.send_cmd("ABOR");
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

pub fn get_file_size(path: &Path) -> usize {
    match lstat(path) {
        Ok(stat) => stat.st_size as usize,
        Err(e) => {
            warn!("Can't get path {:?} state, {}", path, e);
            0
        }
    }
}
