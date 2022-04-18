use crate::handler::cmd::{Answer, ResultCode};
use crate::handler::codec::{BytesCodec, Decoder, Encoder};
use crate::net::connection::Connection;
use log::{debug, info, warn};
use nix::sys::socket::{accept4, setsockopt, sockopt, SockFlag};
use nix::sys::stat::lstat;
use std::fs;
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
    codec: BytesCodec,
    pasv_mode: bool,
}

impl LocalClient {
    pub fn new() -> Self {
        LocalClient {
            hostname: "127.0.0.1".to_string(),
            port: 8089,
            connected: false,
            quit: false,
            cmd_conn: None,
            codec: BytesCodec,
            pasv_mode: true,
        }
    }
    pub fn shell_loop(&mut self) {
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
    pub fn handle_cmd(&mut self, line: &String) -> String {
        // 去回车符号
        let line = strip_trailing_newline(line.clone());
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
            b"PASSIVE" => {
                self.pasv_mode = !self.pasv_mode;
                let on = if self.pasv_mode { "on" } else { "off" };
                println!("Passive mode {}.", on);
            }
            b"ABOR" => self.abort(),
            b"CLOSE" => self.close(),
            b"CD" => self.cd(&args[0]),
            b"LIST" => self.list(&args[0]),
            b"PUT" => self.put(&args[0]),
            b"GET" => self.get(&args[0]),
            b"PWD" => self.pwd(),
            b"MKDIR" => self.mkdir(&args[0]),
            b"RMDIR" => self.rmdir(&args[0]),
            b"DEL" => self.delete(&args[0]),
            b"SYST" => self.syst(),
            b"BINARY" => self.binary(),
            b"SIZE" => self.size(&args[0]),
            b"NOOP" => self.noop(),
            b"HELP" => self.help(),
            b"EXIT" | b"QUIT" => self.exit(),
            _ => println!("?Invalid command"),
        };
        String::from("_")
    }
    fn open(&mut self, args: &Vec<String>) {
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
        let addr = format!("{}:{}", self.hostname, self.port);
        debug!("Connect ftp server: {}", addr);
        self.cmd_conn = Some(Connection::connect(&addr));
        // let welcome = self.receive_answer();
        self.user();
    }
    fn user(&mut self) {
        let mut username = String::new();
        let mut password = String::new();
        print!("Name ({}:user):", self.hostname);
        io::stdout().flush().unwrap();
        stdin().read_line(&mut username).unwrap();
        let username = strip_trailing_newline(username);

        print!("Password:");
        io::stdout().flush().unwrap();
        stdin().read_line(&mut password).unwrap();
        let password = strip_trailing_newline(password);

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
    }
    fn is_open(&self) -> bool {
        self.cmd_conn.is_some()
    }
    fn pwd(&mut self) {
        match self.send_cmd("PWD") {
            Some(answer) => println!("{}", answer),
            None => println!("Failed to get answer"),
        }
    }
    // Example:
    // 200 PORT command successful. Consider using PASV.
    // 150 Here comes the directory listing.
    // -rwxr-xr-x    1 0        0        21863760 Apr 03 20:09 miniftp
    // 226 Directory send OK.
    fn list(&mut self, args: &String) {
        self.port();
        let cmd = format!("LIST {}", args);
        match self.send_cmd(&cmd) {
            Some(answer) => println!("{}", answer),
            None => (),
        }
        let msg = self.receive_data();
        println!("{}", String::from_utf8(msg).unwrap());
    }
    fn receive_data(&mut self) -> Vec<u8> {
        if let Some(mut c) = self.get_data_connect() {
            // ugly function
            let msg = c.read_buf();
            return msg;
        }
        Vec::new()
    }

    fn send_cmd(&mut self, cmd: &str) -> Option<Answer> {
        debug!("Send msg: {}", cmd);
        let buf = cmd.as_bytes().to_vec();
        let mut msg = Vec::new();
        self.codec.encode(buf, &mut msg).unwrap();
        self.cmd_conn.as_mut().unwrap().send(&msg);
        // FIXME: 这个read貌似有bug
        if let Some(ref mut c) = self.cmd_conn {
            let buf = c.read_msg();
            let mut msg = buf.unwrap();

            let answer = self.codec.decode(&mut msg).unwrap();
            return answer;
        }
        None
    }
    fn get_data_connect(&mut self) -> Option<Connection> {
        // TODO: pasv
        self.port()
    }
    fn pasv(&mut self) {
        let s = self.send_cmd("PASV").unwrap();
        println!("{}", s);
    }
    fn port(&mut self) -> Option<Connection> {
        // TODO: check status code
        // FIXME: a connection bug
        // TODO: random port
        let port = self.port + 1;
        let cmd = format!("PORT 127,0,0,1,{},{}", port >> 8, 0xFF & port);
        self.send_cmd(&cmd);
        let addr = format!("{}:{}", "127.0.0.1", port);
        let listener = TcpListener::bind(addr.as_str()).unwrap();
        debug!("listener: {:?}", listener);
        let fd = accept4(listener.as_raw_fd(), SockFlag::SOCK_CLOEXEC).unwrap();
        debug!("accept a new connection: {}", fd);
        setsockopt(fd, sockopt::TcpNoDelay, &true).unwrap();
        debug!("data connection build success");
        Some(Connection::new(fd))
    }

    fn cd(&mut self, path: &String) {
        let msg = format!("CD {}", path);
        match self.send_cmd(&msg) {
            Some(answer) => println!("{}", answer),
            None => (),
        }
    }
    // Example:
    // local: miniftp remote: miniftp
    // 200 PORT command successful. Consider using PASV.
    // 150 Opening BINARY mode data connection for miniftp (21863760 bytes).
    // 226 Transfer complete.
    // 21863760 bytes received in 10.59 secs (1.9698 MB/s)
    fn get(&mut self, file: &String) {
        let mut total_size = 0usize;
        let start = Instant::now();
        if let Some(mut c) = self.get_data_connect() {
            let answer = self.send_cmd(&format!("RETR {}", file)).unwrap();
            println!("{}", answer);
            if answer.code == ResultCode::DataConnOpened {
                let mut fd = fs::File::open(file).unwrap();
                loop {
                    let buf = c.read_buf();
                    if buf.is_empty() {
                        break;
                    }
                    total_size += buf.len();
                    match fd.write(&buf) {
                        Ok(0) | Err(_) => break,
                        Ok(_) => (),
                    }
                }
            }
            println!("total size: {}", total_size);
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
    fn mkdir(&mut self, file: &String) {
        match self.send_cmd(&format!("MKDIR {}", file)) {
            Some(answer) => println!("{}", answer),
            None => (),
        }
    }
    fn rmdir(&mut self, file: &String) {
        match self.send_cmd(&format!("RMDIR {}", file)) {
            Some(answer) => println!("{}", answer),
            None => (),
        }
    }
    fn put(&mut self, file: &String) {
        // local: miniftp remote: miniftp
        // 200 PORT command successful. Consider using PASV.
        // 150 Opening BINARY mode data connection for miniftp (21863760 bytes).
        // 226 Transfer complete.
        // 21863760 bytes received in 10.59 secs (1.9698 MB/s)

        self.binary();
        let mut total_size = 0usize;
        let start = Instant::now();
        let answer = self.send_cmd(&format!("STOR {}", file)).unwrap();
        println!("{}", answer);
        if answer.code != ResultCode::DataConnOpened && answer.code != ResultCode::Ok {
            if let Some(mut c) = self.get_data_connect() {
                total_size += c.send_file(Some(file.as_str()), 0, None, 0).unwrap();
            }
        }
        let duration = start.elapsed();
        let rate = total_size as f64 / 1024f64 / 1024f64 / duration.as_secs_f64();
        debug!("-> file: {:?} transfer done! size: {}", file, total_size);
        println!(
            "{} bytes received in {} secs ({} MB/s)",
            total_size,
            duration.as_secs_f32(),
            rate
        );
    }
    fn delete(&mut self, file: &String) {
        match self.send_cmd(&format!("DELE {}", file)) {
            Some(answer) => println!("{}", answer),
            None => (),
        }
    }
    fn binary(&mut self) {
        match self.send_cmd("TYPE I") {
            Some(answer) => println!("{}", answer),
            None => (),
        }
    }
    fn size(&mut self, file: &String) {
        match self.send_cmd(&format!("SIZE {}", file)) {
            Some(answer) => println!("{}", answer),
            None => (),
        }
    }
    fn syst(&mut self) {
        match self.send_cmd("SYST") {
            Some(answer) => println!("{}", answer),
            None => (),
        }
    }
    fn noop(&mut self) {
        match self.send_cmd("NOOP") {
            Some(answer) => println!("{}", answer),
            None => (),
        }
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
            self.cmd_conn = None;
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

pub fn strip_trailing_newline(s: String) -> String {
    let mut s = s;
    let len_withoutcrlf = s.trim_end().len();
    s.truncate(len_withoutcrlf);
    s
}
