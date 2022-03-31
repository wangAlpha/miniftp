use crate::handler::cmd::{Answer, ResultCode};
use nix::sys::socket::{self, connect, shutdown, socket, sockopt, Shutdown, SockFlag};
use nix::sys::socket::{accept4, listen, setsockopt};
use nix::sys::socket::{AddressFamily, InetAddr, SockAddr, SockProtocol, SockType};
use nix::unistd::{read, write};
use std::io::{self, stdin, Read, Write};
use std::net::SocketAddr;
use std::net::TcpListener;
use std::os::unix::prelude::AsRawFd;
use std::str::FromStr;

use super::connection::Connection;

#[derive(Debug)]
pub struct LocalClient {
    hostname: String,
    port: u16,
    conn_fd: i32,
    connected: bool,
    data_conn: Option<Connection>,
}

impl LocalClient {
    pub fn new() -> Self {
        let sockfd = socket(
            AddressFamily::Inet,
            SockType::Stream,
            SockFlag::SOCK_CLOEXEC,
            SockProtocol::Tcp,
        )
        .unwrap();
        LocalClient {
            hostname: String::new(),
            port: 8089,
            conn_fd: sockfd,
            connected: false,
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
        let (mut cmd, args) = (commands[0].to_uppercase(), &commands[1..]);
        match cmd.as_bytes() {
            b"OPEN" => self.open(args),
            b"USER" => self.user(),
            b"PASV" => {
                let s = self.send_cmd(String::from_str("PASV").unwrap());
                println!("{}", s);
            }
            b"PORT" => self.port(),
            b"CLOSE" => self.close(),
            b"CD" => self.cd(),
            b"LS" => self.list(),
            b"PUT" => self.put(),
            b"GET" => self.get(),
            b"PWD" => self.pwd(),
            b"MKDIR" => self.mkdir(),
            b"RMDIR" => self.rmdir(),
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

        let addr = SocketAddr::from_str("127.0.0.1:8089").unwrap();
        let inet_addr = InetAddr::from_std(&addr);
        let sock_addr = SockAddr::new_inet(inet_addr);
        let username = String::new();
        let password = String::new();
        match connect(self.conn_fd, &sock_addr) {
            Ok(()) => println!("Connection success!"),
            Err(e) => println!("Connection failed: {}", e),
        }
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
    fn send_cmd(&self, cmd: String) -> String {
        match write(self.conn_fd, cmd.as_bytes()) {
            Ok(0) => println!("Invalid command"),
            Ok(n) => {}
            Err(_) => println!("Invalid command"),
        }
        let mut buf: Vec<u8> = Vec::new();
        match read(self.conn_fd, &mut buf) {
            Ok(0) => println!("can't recevide cmd"),
            Ok(n) => {
                println!("result {:?}", buf);
            }
            Err(err) => {
                println!("error: {}", err);
            }
        }
        String::from_utf8_lossy(&buf).to_string()
    }
    // fn parse_result(&mut self, msg: &String) {
    //     let iter= msg.split_ascii_whitespace().next().unwrap();
    //     let mut message = String::new();
    //     Answer {
    //         code: code.parse().unwrap(),
    //         message:
    //     }
    // }
    fn port(&mut self) {
        // TODO: check status code
        if let Some(c) = self.data_conn.clone() {
            c.shutdown();
            self.data_conn = None;
        }
        // TODO file configure
        self.send_cmd(String::from("PORT 127,0,0,1,31,154\r\n"));
        let addr = format!("{}:{}", "127.0.0.1", 8090);
        let listener = TcpListener::bind(addr.as_str()).unwrap();
        let fd = accept4(listener.as_raw_fd(), SockFlag::SOCK_CLOEXEC).unwrap();
        setsockopt(fd, sockopt::TcpNoDelay, &true).unwrap();
        self.data_conn = Some(Connection::new(fd));
    }

    fn cd(&mut self) {}
    fn put(&mut self) {}
    fn upload(&mut self) {}
    fn get(&mut self) {}
    fn pwd(&mut self) {}
    fn mkdir(&mut self) {}
    fn list(&mut self) {}
    fn rmdir(&mut self) {}
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
            shutdown(self.conn_fd, Shutdown::Both).expect("can't shutdown connection");
        }
    }
}
