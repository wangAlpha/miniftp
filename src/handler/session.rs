use crate::handler::codec::{Decoder, Encoder, FtpCodec};
use crate::handler::speed_barrier::SpeedBarrier;
use crate::net::acceptor::Acceptor;
use crate::net::connection::Connection;
use crate::net::event_loop::EventLoop;
use crate::net::socket::Socket;
use crate::server::record_lock::FileLock;
use crate::utils::config::Config;
use crate::utils::utils::is_regular;
use crate::{handler::cmd::*, utils::utils::is_exist};
use crate::{is_blk, is_char, is_dir, is_link, is_pipe, is_reg, is_sock};
use chrono::prelude::*;
use log::{debug, info, warn};
use nix::dir::{Dir, Type};
use nix::fcntl::{open, renameat, OFlag};
use nix::sys::epoll::EpollFlags;
use nix::sys::stat::{fchmodat, lstat, FchmodatFlags, Mode, SFlag};
use nix::sys::utsname::uname;
use nix::unistd::{close, ftruncate, lseek, mkdir, unlink, write};
use nix::unistd::{Gid, Group, Uid, User, Whence};
use std::fs::canonicalize;
use std::os::unix::prelude::AsRawFd;
use std::path::{Component, Path, PathBuf};
use std::string::String;
use std::time::Instant;
use std::{collections::HashMap, net::TcpListener};

pub const KILOGYTE: f64 = 1024f64;
pub const MEGA_BYTE: f64 = KILOGYTE * 1024f64;
pub const GIGA_BYTE: f64 = MEGA_BYTE * 1024f64;

const DEFAULT_DIR_PERM: u32 = 0x777;
const DEAFULT_FILE_PERM: u32 = 0x666;
const DEAFULT_SEND_SIZE: usize = 128 * 1024; // bytes

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
    cur_dir: PathBuf,
    file_name: Option<String>,
    cmd_conn: Connection,
    data_conn: Option<Connection>,
    data_port: Option<u16>,
    codec: FtpCodec,
    server_root: PathBuf,
    mode: u32, // for umask mode
    name: Option<String>,
    is_admin: bool,
    transfer_type: TransferType,
    waiting_password: bool,
    event_loop: EventLoop,
    config: Config,
    pasv_enable: bool,
    welcome: bool,
    resume_point: i64,
    help_map: HashMap<&'static str, &'static str>,
}

impl Session {
    pub fn new(config: &Config, conn: Connection, event_loop: &EventLoop) -> Self {
        let root = User::from_uid(Uid::from_raw(0)).unwrap().unwrap();
        Session {
            cur_dir: canonicalize(root.dir.clone()).unwrap(),
            file_name: None,
            cmd_conn: conn,
            data_conn: None,
            data_port: Some(22),
            codec: FtpCodec,
            server_root: canonicalize(root.dir.clone()).unwrap(),
            mode: 0x0,
            is_admin: true,
            transfer_type: TransferType::BINARY,
            waiting_password: false,
            event_loop: event_loop.clone(),
            name: None,
            config: config.clone(),
            pasv_enable: config.pasv_enable,
            welcome: true,
            resume_point: 0,
            help_map: Self::get_help_map(),
        }
    }
    pub fn handle_command(&mut self) {
        // if revents.is_reable()
        if !self.cmd_conn.connected() {
            debug!("Session command is disconnnectd");
            return;
        }
        if self.welcome {
            self.welcome = false;
            self.send_answer(Answer::new(
                ResultCode::ServiceReadyForUsr,
                "Welcome, tinyFTPd 3.0.3)",
            ));
        }
        let msg = self.cmd_conn.read_msg();
        if msg.is_none() {
            return;
        }
        let mut msg = msg.unwrap();
        let cmd = self.codec.decode(&mut msg).unwrap().unwrap();
        info!(
            "A connection ({}->{}) command: {:?}",
            self.cmd_conn.get_peer_addr(),
            self.cmd_conn.get_local_addr(),
            cmd
        );
        if self.is_logged() {
            match cmd.clone() {
                // Access control commands
                Command::Cwd(dir) => self.cwd(self.to_absolute(dir)),
                Command::CdUp => self.cdup(),
                // Transfer parameter commands
                Command::Port(port) => self.port(port),
                Command::Pasv => self.pasv(),
                Command::Type(typ) => {
                    self.transfer_type = typ;
                    let message = format!("Opening {} mode to transfer files.", typ);
                    self.send_answer(Answer::new(ResultCode::Ok, &message));
                }
                // Query commands
                Command::List(path) => self.list(path, true),
                Command::NLst(path) => self.list(path, false),
                Command::Pwd => self.pwd(),
                Command::Size(path) => self.size(self.to_absolute(path)),
                Command::Help(content) => self.help(content),
                // File control commands
                Command::Stor(path) => self.stor(self.to_absolute(path)),
                Command::Retr(path) => self.retr(self.to_absolute(path)),
                Command::Mkd(path) => self.mkd(self.to_absolute(path)),
                Command::Rmd(path) => self.rmd(self.to_absolute(path)),
                Command::Delete(path) => self.delete(self.to_absolute(path)),
                Command::Rnfr(path) => self.rnfr(self.to_absolute(path)),
                Command::Rnto(path) => self.rnto(self.to_absolute(path)),
                Command::Site(contents) => self.site(contents),
                Command::Rest(content) => self.rest(content),
                // Others commands
                Command::Abort => self.abort(),
                _ => (),
            }
        } else if let Command::Pass(content) = cmd.clone() {
            self.pass(content);
        }

        match cmd {
            // Access control commands
            Command::User(content) => self.user(content),
            Command::Quit => self.quit(),
            Command::Syst => {
                let sys = uname();
                let message = format!(
                    "{} {} {} {} {}",
                    sys.sysname(),
                    sys.nodename(),
                    sys.release(),
                    sys.version(),
                    sys.machine(),
                );
                self.send_answer(Answer::new(ResultCode::Ok, &message));
            }
            Command::Acct => {
                self.send_answer(Answer::new(ResultCode::CmdNotImpl, "Not implemented"))
            }
            Command::NoOp => self.send_answer(Answer::new(ResultCode::Ok, "Doing nothing")),
            Command::Unknown(s) => {
                self.send_answer(Answer::new(
                    ResultCode::SyntaxErr,
                    &format!("\"{}\": not implemented", s),
                ));
            }
            _ => (),
        }
    }
    // TODO: check passwd, and cd to current user directory
    fn pass(&mut self, content: String) {
        let ok = if self.is_admin {
            content.eq(&self.config.users[&self.name.clone().unwrap()])
        } else {
            content.eq(&self.config.users[&(self.name.clone().unwrap())])
        };
        if ok {
            self.waiting_password = false;
            self.send_answer(Answer::new(
                ResultCode::Login,
                &format!("Welcome {}", self.name.clone().unwrap()),
            ));
        } else {
            self.send_answer(Answer::new(ResultCode::LongPassMode, "Invalid password...."));
        }
    }
    // TODO: check passwd, and cd to current user directory
    fn user(&mut self, content: String) {
        if content.is_empty() {
            self.send_answer(Answer::new(ResultCode::CmdNotCmplParam, "Invaild username"));
        } else {
            let mut name: Option<String> = None;
            let mut pass_required = true;
            self.is_admin = false;

            if let Some(ref admin) = self.config.admin {
                if content.eq(admin) {
                    self.is_admin = true;
                    name = Some(content.clone());
                    pass_required = !self.config.users[&(content.clone())].is_empty();
                }
            }
            if name.is_none() {
                if self.config.users.contains_key(&content) {
                    name = Some(content.clone());
                    pass_required = !self.config.users[&(content.clone())].is_empty()
                }
            }
            if name.is_none() {
                self.send_answer(Answer::new(ResultCode::NotLogin, "Unknown user..."));
            } else {
                self.name = name.clone();
                if pass_required {
                    self.waiting_password = true;
                    self.send_answer(Answer::new(
                        ResultCode::NeedPsw,
                        &format!("Login Ok, password needed for {}", name.clone().unwrap()),
                    ));
                } else {
                    self.waiting_password = false;
                    let message = format!("Login successful.");
                    self.send_answer(Answer::new(ResultCode::Login, &message));
                }
            }
            let user_dir = Path::new("/home").join(name.clone().unwrap_or(String::new()));
            self.cur_dir = if user_dir.exists() {
                user_dir
            } else {
                self.cur_dir.join(name.clone().unwrap_or(String::new()))
            };
        }
        info!(
            "user: {}, current directory: {:?}",
            self.name.clone().unwrap_or(String::new()),
            self.cur_dir
        );
    }
    pub fn get_data_conn(&mut self) -> Option<Connection> {
        let port = if let Some(port) = self.data_port { port } else { 22 };
        if self.pasv_enable {
            let c = self.data_conn.to_owned();
            self.data_conn = None;
            return c;
        } else {
            let addr = format!("127.0.0.1:{}", port);
            let mut sock = Socket::connect(&addr);
            sock.set_keep_alive(true);
            sock.set_no_delay(true);
            sock.set_reuse_addr(true);
            sock.set_reuse_port(true);
            return Some(Connection::new(sock));
        }
    }
    pub fn set_revents(&mut self, revents: &EpollFlags) {
        self.cmd_conn.set_revents(revents);
    }
    fn is_logged(&self) -> bool {
        self.name.is_some() && !self.waiting_password
    }
    fn to_absolute(&self, path: PathBuf) -> PathBuf {
        if path.is_relative() {
            return self.cur_dir.join(path);
        }
        path
    }
    fn mkd(&mut self, path: PathBuf) {
        let mut ok = false;
        let path = path.to_str().unwrap();
        if self.is_admin && !is_exist(path) {
            match mkdir(path, Mode::all()) {
                Ok(_) => {
                    debug!("created {:?}", path);
                    ok = true;
                }
                Err(e) => println!("Error creating directory: {}", e),
            }
        }
        if ok {
            let message = &format!("Folder {} successfully created!", path);
            self.send_answer(Answer::new(ResultCode::FileActOk, &message));
        } else {
            self.send_answer(Answer::new(ResultCode::FileNameNotAllow, "Couldn't create folder"));
        }
    }
    fn rmd(&mut self, path: PathBuf) {
        // check path
        if self.is_admin
            && is_exist(path.to_str().unwrap_or(""))
            && path.is_dir()
            && remove_dir_all(&path)
        {
            self.send_answer(Answer::new(ResultCode::FileActOk, "Folder successufully removed"));
        } else {
            self.send_answer(Answer::new(ResultCode::FileNotFound, "Couldn't remove folder"));
        }
    }
    fn delete(&mut self, path: PathBuf) {
        let mut ok = false;
        let path = if path.is_absolute() { path } else { self.cur_dir.join(path) };
        if is_exist(path.to_str().unwrap()) && is_regular(path.to_str().unwrap()) {
            match unlink(&path) {
                Ok(_) => ok = true,
                Err(e) => warn!("Couln't remove file, Err: {}", e),
            }
        }
        let name = path.file_name().unwrap().to_str().unwrap();
        if ok {
            self.send_answer(Answer::new(
                ResultCode::FileActOk,
                &format!("File {} successufully removed", name),
            ));
        } else {
            self.send_answer(Answer::new(
                ResultCode::FileNotFound,
                &format!("Couldn't remove file {}", name),
            ));
        }
    }
    fn rnfr(&mut self, path: PathBuf) {
        let file_name = path.to_str().unwrap();
        if self.is_admin && is_exist(file_name) && is_regular(file_name) {
            self.file_name = Some(file_name.to_string());
            self.send_answer(Answer::new(
                ResultCode::FileActionPending,
                &format!("Ready for rename file {}", file_name),
            ));
        } else {
            self.send_answer(Answer::new(
                ResultCode::FileNotFound,
                &format!("Couldn't rename file {}", file_name),
            ));
        }
    }
    fn rnto(&mut self, path: PathBuf) {
        let mut ok = false;
        let new_file = path.as_path().to_string_lossy().to_string();
        let old_path = self.file_name.clone();
        if let Some(ref old_file) = self.file_name {
            if is_exist(old_file.as_str()) && is_regular(old_file.as_str()) {
                ok = renameat(None, Path::new(&old_file), None, Path::new(&new_file)).is_ok();
            }
        }
        self.file_name = None;
        if ok {
            let message =
                format!("Rename file {} successful rename to {}", old_path.unwrap(), new_file);
            self.send_answer(Answer::new(ResultCode::FileActOk, &message));
        } else {
            self.send_answer(Answer::new(ResultCode::FileNameNotAllow, "Coldn't rename file"));
        }
    }
    fn site(&mut self, contents: Vec<String>) {
        debug!("Site: {:?}", contents);
        let mut ok = false;
        if contents.len() == 2 && contents[0] == "umask" {
            if let Ok(mode) = contents[1].parse::<u32>() {
                self.mode = mode;
                ok = true;
                self.send_answer(Answer::new(ResultCode::Ok, &format!("UMASK set to {}", mode)));
            }
        } else if contents.len() == 3 && contents[0] == "chmod" {
            if let Ok(mode) = contents[1].parse::<u32>() {
                ok = fchmodat(
                    None,
                    Path::new(&contents[2]),
                    Mode::from_bits(mode).unwrap_or(Mode::all()),
                    FchmodatFlags::NoFollowSymlink,
                )
                .is_ok();
                if ok {
                    let message = format!("chmod {} {}", mode, contents[2]);
                    self.send_answer(Answer::new(ResultCode::Ok, &message));
                }
            }
        }
        if !ok {
            let message = format!("Unknown site command: {:?}.", contents);
            self.send_answer(Answer::new(ResultCode::BadCmdSeq, &message));
        }
    }

    fn rest(&mut self, content: String) {
        if let Ok(n) = content.parse::<i64>() {
            self.resume_point = n;
            let message =
                format!("Restarting at {}. execute get, put or append to initiate transfer", n);
            self.send_answer(Answer::new(ResultCode::FileActionPending, &message));
        } else {
            self.send_answer(Answer::new(ResultCode::BadCmdSeq, "Couldn't restart break point"));
        }
    }
    fn cwd(&mut self, dir: PathBuf) {
        let directory = dir.to_str().unwrap();
        let mut ok = false;
        // check path invalid and exist
        if is_exist(directory) && dir.is_dir() {
            if let Ok(dir) = canonicalize(dir.clone()) {
                let current_dir = dir;
                self.cur_dir = current_dir;
                ok = true;
            }
        }
        if ok {
            self.send_answer(Answer::new(
                ResultCode::FileActOk,
                "Change current path successfully",
            ));
        } else {
            self.send_answer(Answer::new(ResultCode::FileNotFound, "No such file or directory"));
        }
    }
    fn cdup(&mut self) {
        let mut ok = false;
        let dir = self.cur_dir.join("..");
        if let Ok(dir) = canonicalize(dir) {
            let current_dir = dir;
            self.cur_dir = current_dir;
            ok = true;
        }
        if ok {
            self.send_answer(Answer::new(
                ResultCode::FileActOk,
                "Change current path successfully",
            ));
        } else {
            self.send_answer(Answer::new(ResultCode::FileNotFound, "No such file or directory"));
        }
    }
    fn list(&mut self, path: Option<PathBuf>, add_info: bool) {
        let path = path.unwrap_or(PathBuf::from("."));
        if let Some(mut c) = self.get_data_conn() {
            let path = self.to_absolute(path);
            if !is_exist(path.as_path().to_str().unwrap_or("")) {
                self.send_answer(Answer::new(ResultCode::FileNotFound, "File not found"));
                return;
            }
            self.send_answer(Answer::new(
                ResultCode::FileStatusOk,
                "Starting to list directory...",
            ));
            let mut out = Vec::new();
            if path.is_dir() {
                let dir = Dir::open(path.as_os_str(), OFlag::O_DIRECTORY, Mode::S_IXUSR).unwrap();
                let mut file_names = dir
                    .into_iter()
                    .map(|entry| entry.unwrap().file_name().to_string_lossy().to_string())
                    .collect::<Vec<String>>();
                file_names.sort();
                file_names = file_names
                    .iter()
                    .map(|x| self.cur_dir.join(x).to_string_lossy().to_string())
                    .collect::<Vec<String>>();
                if add_info {
                    file_names.iter().for_each(|s| add_file_info(s, &mut out));
                } else {
                    file_names
                        .iter()
                        .map(|s| s.split('/').last().unwrap())
                        .filter(|&s| s.as_bytes()[0] as char != '.')
                        .for_each(|s| out.extend(format!("{}\r\n", s).as_bytes()));
                }
            } else {
                let path = path.as_os_str().to_str().unwrap();
                if add_info {
                    add_file_info(path, &mut out);
                } else {
                    out.extend(format!("{}\r\n", path).as_bytes());
                }
            }
            c.send(&out);
            c.shutdown();
            self.send_answer(Answer::new(ResultCode::CloseDataClose, "Directory send Ok"));
        } else {
            self.send_answer(Answer::new(ResultCode::ConnClose, "No opened data connection"));
        }
    }
    fn pasv(&mut self) {
        self.pasv_enable = true;
        let port = if let Some(port) = self.data_port { port + 2 } else { 22 };
        let message = format!("Entering Passive Mode (127,0,0,1,{},{})", port >> 8, port & 0xFF);
        let addr = format!("0.0.0.0:{}", port);
        // TODO: Add connection fail handler
        let listener = TcpListener::bind(&addr).unwrap();
        self.send_answer(Answer::new(ResultCode::PassMode, &message));
        let mut s = Acceptor::accept(listener.as_raw_fd());
        s.set_keep_alive(true);
        s.set_no_delay(true);
        s.set_reuse_addr(true);
        s.set_reuse_port(true);
        self.data_conn = Some(Connection::new(s));
    }
    fn port(&mut self, port: u16) {
        self.pasv_enable = false;
        self.data_port = Some(port);
        let message = format!("PORT command successful, data port is now {}", port);
        self.send_answer(Answer::new(ResultCode::Ok, &message));
    }
    fn size(&mut self, path: PathBuf) {
        let mut size = None;
        // Check file whether exist
        let file_name = path.to_string_lossy().to_string();
        if is_exist(file_name.as_str()) && is_regular(file_name.as_str()) {
            if let Ok(stat) = lstat(&path) {
                size = Some(stat.st_size);
            }
        }
        if let Some(size) = size {
            let message = format!("{}", size);
            self.send_answer(Answer::new(ResultCode::FileStatus, &message));
        } else {
            self.send_answer(Answer::new(ResultCode::FileNotFound, "Could not get file size."));
        }
    }
    fn pwd(&mut self) {
        let message = format!("{}", self.cur_dir.to_str().unwrap_or(""));
        if !message.is_empty() {
            let msg = format!("\"{}\"", message);
            self.send_answer(Answer::new(ResultCode::CreatPath, msg.as_str()));
        } else {
            self.send_answer(Answer::new(ResultCode::FileNotFound, "No such file or directory"));
        }
    }
    fn quit(&mut self) {
        self.send_answer(Answer::new(ResultCode::ServiceCloseCtlCon, "Goodbye"));
        self.cmd_conn.shutdown();
    }
    fn retr(&mut self, path: PathBuf) {
        // 21863760 bytes received in 0.30 secs (70.3109 MB/s)
        if let Some(mut c) = self.get_data_conn() {
            let path = path.to_str().unwrap();
            let mode = self.transfer_type;
            let mut ok = false;
            if is_exist(path) && is_regular(path) && self.is_admin {
                let message = format!("Opening {} mode data connection for {}", mode, &path);
                self.send_answer(Answer::new(ResultCode::FileStatusOk, &message));
                let instant = Instant::now();
                if let Ok(fd) = open(path, OFlag::O_RDWR, Mode::S_IRUSR) {
                    ok = true;
                    let size = lstat(path).unwrap().st_size as usize;
                    let mut barrier = SpeedBarrier::new(self.config.max_speed);
                    let mut len = 0usize;
                    loop {
                        match c.send_file(None, fd, Some(len as i64), size) {
                            Some(0) => break,
                            Some(n) => {
                                len += n;
                                if n < DEAFULT_SEND_SIZE {
                                    break;
                                }
                                barrier.limit_speed(n);
                            }
                            None => {
                                warn!("Can't send file {}", path);
                                break;
                            }
                        }
                    }
                    c.shutdown();
                    let message = format!("Transfer {} complete", path);
                    self.send_answer(Answer::new(ResultCode::CloseDataClose, &message));
                    info!("Transfer {} complete", path);
                    let elapsed = instant.elapsed().as_secs_f64();
                    let size = format_size(len as f64 / elapsed);
                    info!("{} bytes send in {:.2} secs ({}B/s)", len, elapsed, size);
                    info!("-> file transfer done!");
                }
            }
            if !ok {
                self.send_answer(Answer::new(
                    ResultCode::FileNotFound,
                    &format!("Failed to open file {}, please check file", path),
                ));
            }
            c.shutdown();
        } else {
            self.send_answer(Answer::new(ResultCode::ConnClose, "No opened data connection"));
        }
    }
    // example:
    // local: hello remote: miniftp
    // 200 PORT command successful. Consider using PASV.
    // 150 Opening BINARY mode data connection for miniftp (21863760 bytes).
    // 226 Transfer complete.
    // 21863760 bytes received in 10.81 secs (1.9284 MB/s)
    fn stor(&mut self, path: PathBuf) {
        if let Some(mut c) = self.get_data_conn() {
            // check file path and admin

            if self.is_admin {
                self.send_answer(Answer::new(
                    ResultCode::DataConnOpened,
                    "Starting to receive file...",
                ));
                let path = path.to_str().unwrap();
                let oflag: OFlag = OFlag::O_CREAT | OFlag::O_RDWR;

                let fd = open(path, oflag, Mode::from_bits(DEAFULT_FILE_PERM).unwrap()).unwrap();
                let _lock = FileLock::new(fd).lock(true);
                if self.resume_point <= 0 {
                    ftruncate(fd, 0).expect("Couldn't ftruncate file at 0");
                    lseek(fd, 0, Whence::SeekSet)
                        .expect(&format!("Couldn't lseek file: {} at {}", path, 0));
                } else {
                    lseek(fd, self.resume_point, Whence::SeekSet)
                        .expect(&format!("Couldn't lseek file: {} at {}", path, self.resume_point));
                    self.resume_point = 0;
                }
                let instant = Instant::now();
                let mut len = 0usize;
                let mut barrier = SpeedBarrier::new(self.config.max_speed);
                loop {
                    let buf = c.read_buf();
                    if buf.is_empty() {
                        break;
                    }
                    match write(fd, &buf) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => {
                            len += n;
                            debug!("Receive data {}", buf.len());
                        }
                    }
                    barrier.limit_speed(buf.len());
                }
                close(fd).unwrap();
                let elapsed = instant.elapsed().as_secs_f64();
                let size = format_size(len as f64 / elapsed);
                info!("{} bytes received in {:.2} secs ({}B/s)", len, elapsed, size);
                c.shutdown();
                self.send_answer(Answer::new(
                    ResultCode::CloseDataClose,
                    &format!("Transfer file {} done", path),
                ));
            } else {
                c.shutdown();
                self.send_answer(Answer::new(ResultCode::FileNotFound, "Couldn't open file"));
            }
        } else {
            self.send_answer(Answer::new(ResultCode::DataConnFail, "No opened data connection"));
        }
    }
    fn help(&mut self, content: String) {
        if self.help_map.contains_key(&content.as_str()) {
            let message = self.help_map[&content.as_str()];
            self.send_answer(Answer::new(ResultCode::HelpMsg, &message));
        } else {
            self.send_answer(Answer::new(
                ResultCode::SyntaxErr,
                &format!("?Invalid help command {}", content),
            ));
        }
    }
    fn get_help_map() -> HashMap<&'static str, &'static str> {
        HashMap::from([
            ("open", "open hostname [ port ] - open new connection"),
            ("user", "user username - send new user information"),
            ("cd", "cd remote-directory - change remote working directory"),
            ("ls", "ls [ remote-directory ] - print list of files in the remote directory"),
            ("put", "put local-file [ remote-file ] - store a file at the server"),
            ("pwd", "get remote-file [ local-file ] - retrieve a copy of the file"),
            ("mkdir", "pwd - print the current working directory name"),
            ("rmdir", "mkdir directory-name - make a directory on the remote machine"),
            ("del", "rmdir directory-name - remove a directory"),
            ("del", "del remote-file - delete a file"),
            ("binary", "binary - set binary transfer type"),
            ("size", "size remote-file - show size of remote file"),
            ("stat", "stat [ remote-file ] - print server information"),
            ("syst", "syst - show remote system type"),
            ("noop", "noop - no operation"),
            ("close", "close - close current connection"),
            ("help", "help - print list of ftp commands"),
            ("exit", "exit - exit program"),
        ])
    }
    fn abort(&mut self) {
        self.send_answer(Answer::new(ResultCode::CloseDataClose, "No transfer to Abort!"));
    }
    fn send_answer(&mut self, answer: Answer) {
        let mut buf = Vec::new();
        self.codec.encode(answer.clone(), &mut buf).unwrap();
        self.cmd_conn.send(&buf);
        debug!("{} {}", answer, buf.len());
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        // let fd = self.cmd_conn.get_fd();
        self.cmd_conn.shutdown();
    }
}

fn invaild_path(path: &Path) -> bool {
    for component in path.components() {
        if let Component::ParentDir = component {
            return true;
        }
    }
    true
}

pub fn permissions(mode: u32) -> String {
    let mut out = b"wrxwrxwrx".to_vec();
    for i in 0..9 {
        if mode & (1 << i) == 0 {
            out[i] = b'-';
        }
    }
    String::from_utf8(out).unwrap()
}

// Output directoty information, example:
// drwxr-xr-x 19 root root 646 Apr  3 12:14 ..
// drwxr-xr-x  8 root root 272 Mar 29 20:33 handler/
// -rw-r--r--  1 root root 168 Mar 28 17:49 lib.rs
// FIXME: fix to user name case
pub fn add_file_info(path: &str, out: &mut Vec<u8>) {
    if path.is_empty() {
        return;
    }
    let stat = lstat(path).unwrap();
    let mode = stat.st_mode;
    let file_typ = if is_reg!(mode) {
        "-"
    } else if is_link!(mode) {
        "l"
    } else if is_dir!(mode) {
        "d"
    } else if is_sock!(mode) {
        "s"
    } else if is_char!(mode) {
        "c"
    } else if is_blk!(mode) {
        "b"
    } else if is_pipe!(mode) {
        "p"
    } else {
        "?"
    };

    let extra = if is_dir!(mode) { "/" } else { "" };
    // match
    // {mouth} {day} {hour}:{min}
    let naive = NaiveDateTime::from_timestamp(stat.st_ctime, 0);
    let datetime: DateTime<Utc> = DateTime::from_utc(naive, Utc);
    let time = datetime.format("%b %d %X").to_string();

    let size = format_size(stat.st_size as f64);
    let rights = permissions(stat.st_mode);
    let links = stat.st_nlink;
    let user = User::from_uid(Uid::from_raw(stat.st_uid)).unwrap().unwrap();
    let group = Group::from_gid(Gid::from_raw(stat.st_gid)).unwrap().unwrap();
    let path = path.split('/').last().unwrap();

    let file_str = format!(
        "{file_typ}{rights} {links:3} {owner} {group} {size}  {time} {path}{extra}\r\n",
        file_typ = file_typ,
        rights = rights,
        links = links,
        owner = user.name,
        group = group.name,
        size = size,
        time = time,
        path = path,
        extra = extra,
    );
    out.extend(file_str.as_bytes());
    print!("==> {}", file_str);
}

pub fn format_size(st_size: f64) -> String {
    let size = if st_size > GIGA_BYTE {
        format!("{:6.2}G", st_size / GIGA_BYTE)
    } else if st_size > MEGA_BYTE {
        format!("{:6.2}M", st_size / MEGA_BYTE)
    } else if st_size >= KILOGYTE {
        format!("{:6.2}K", st_size / KILOGYTE)
    } else {
        format!("{:7}", st_size)
    };
    size
}

pub fn remove_dir_all(path: &Path) -> bool {
    if !path.is_dir() {
        return false;
    }
    let dir = Dir::open(path, OFlag::O_DIRECTORY, Mode::S_IXUSR).unwrap();
    for entry in dir.into_iter() {
        let entry = entry.unwrap();
        let file_name = entry.file_name().to_string_lossy().to_string();
        let path = Path::new(entry.file_name().to_str().unwrap());
        let file_type = entry.file_type().unwrap();
        if file_name != "." && file_name != ".." {
            if file_type == Type::Directory {
                remove_dir_all(path);
            } else {
                unlink(path).expect(&format!("Couldn't unlink file {}", path.display()));
            }
        }
    }
    unlink(path).expect(&format!("Couldn't unlink file {}", path.display()));
    true
}
