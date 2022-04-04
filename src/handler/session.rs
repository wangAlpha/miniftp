use crate::handler::cmd::*;
use crate::handler::codec::FtpCodec;
use crate::net::connection::{ConnRef, Connection, State};
use crate::net::event_loop::EventLoop;
use crate::server::local_client::*;
use crate::utils::config::Config;
use chrono::prelude::*;
use log::{debug, info, warn};
use nix::dir::{Dir, Type};
use nix::fcntl::{open, OFlag};
use nix::sys::stat::{lstat, Mode, SFlag};
use nix::sys::utsname::uname;
use nix::unistd::{chdir, close, mkdir, read, unlink, write};
use std::collections::HashMap;
use std::env::current_dir;
use std::iter::Iterator;
use std::net::TcpListener;
use std::os::unix::prelude::AsRawFd;
use std::path::{Component, Path, PathBuf};
use std::string::String;
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
    codec: FtpCodec,
    server_root: PathBuf,
    name: Option<String>,
    is_admin: bool,
    transfer_type: TransferType,
    curr_file_context: Option<Context>,
    waiting_password: bool,
    event_loop: EventLoop,
    config: Config,
}

impl Session {
    pub fn new(config: &Config, conn: ConnRef, event_loop: EventLoop) -> Self {
        Session {
            cwd: PathBuf::new(),
            cmd_conn: conn,
            data_conn: None,
            data_port: Some(2222),
            codec: FtpCodec,
            server_root: current_dir().unwrap(),
            is_admin: true,
            transfer_type: TransferType::ASCII,
            waiting_password: false,
            event_loop,
            curr_file_context: None,
            name: None,
            config: config.clone(),
        }
    }
    pub fn handle_command(&mut self, msg: Vec<u8>, conn_map: &mut Arc<Mutex<HashMap<i32, i32>>>) {
        let cmd = Command::new(msg).unwrap();
        let connected = self.cmd_conn.lock().unwrap().connected();
        if connected && self.is_logged() {
            match cmd {
                Command::Cwd(dir) => self.cwd(dir),
                Command::List(path) => self.list(path),
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
                    conn_map.lock().unwrap().insert(data_fd, cmd_fd);
                }
                Command::Pwd => {
                    self.pwd();
                }
                Command::Stor(path) => {
                    self.stor(path);
                }
                Command::CdUp => {
                    let path = self.cwd.as_path();
                    // TODO: Cd pathbuf
                    // self.cwd = chdir(path).unwrap();
                    // get_path
                }
                Command::Mkd(path) => self.mkd(path),
                Command::Retr(path) => self.retr(path),
                Command::Rmd(path) => self.rmd(path),
                // TODO: Command::Rename(srd, dst) =>
                _ => (),
            }
        } else if self.name.is_some() && self.waiting_password {
            if let Command::Pass(content) = cmd {
                let mut ok = false;
                if self.is_admin {
                    ok = content.eq(&self.config.admin.clone().unwrap());
                } else {
                    ok = self.config.users[&(self.name.clone().unwrap())] == content;
                }
                if ok {
                    self.waiting_password = false;
                    self.send_answer(Answer::new(
                        ResultCode::Login,
                        &format!("Welcome {}", self.name.clone().unwrap()),
                    ));
                } else {
                    self.send_answer(Answer::new(
                        ResultCode::LongPassMode,
                        "Invalid password....",
                    ));
                }
            }
        } else {
            match cmd {
                Command::Auth => {
                    self.send_answer(Answer::new(ResultCode::CmdNotImpl, "Not implemented"))
                }
                Command::Quit => self.quit(),
                Command::User(content) => {
                    if content.is_empty() {
                        self.send_answer(Answer::new(
                            ResultCode::CmdNotCmplParam,
                            "Invaild username",
                        ));
                    } else {
                        let mut name: Option<String> = None;
                        let mut pass_required = true;
                        self.is_admin = false;

                        if let Some(ref admin) = self.config.admin {
                            self.is_admin = admin.eq(&self.config.admin.clone().unwrap());
                        }
                        if self.name.is_none() {
                            if self.config.users.contains_key(&content) {
                                self.name = Some(content.clone());
                                pass_required =
                                    self.config.users[&(content.clone())].is_empty() == false
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
                                    &format!(
                                        "Login Ok, password needed for {}",
                                        name.unwrap().clone()
                                    ),
                                ));
                            } else {
                                self.waiting_password = false;
                                let typ = if self.transfer_type == TransferType::BINARY {
                                    "binary"
                                } else {
                                    "ascii"
                                };
                                let welcome = format!(
                                    "Login successful.\r\n
                                Welcome {}\r\n
                                Remote system type is UNIX.\r\n
                                Using {} mode to transfer files.\r\n",
                                    typ,
                                    name.unwrap().clone()
                                );
                                self.send_answer(Answer::new(ResultCode::Login, &welcome));
                            }
                        }
                    }
                    self.send_answer(Answer::new(ResultCode::Ok, "Login success"))
                }
                Command::Type(typ) => {
                    self.transfer_type = typ;
                    let mode = if typ == TransferType::ASCII {
                        "ASCII"
                    } else {
                        "BINARY"
                    };
                    let msg = format!("Using {} mode to transfer files.", mode);
                    self.send_answer(Answer::new(ResultCode::Ok, &msg));
                }
                Command::Syst => {
                    self.send_answer(Answer::new(ResultCode::Ok, &format!("{:?}", uname())));
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
        debug!("cmd_conn: {} is logged: {}", connected, self.is_logged());
    }
    fn is_logged(&self) -> bool {
        self.name.is_some() && self.waiting_password
    }
    fn mkd(&mut self, path: PathBuf) {
        let path = self.cwd.join(&path);

        match mkdir(&path, Mode::S_IRWXU) {
            Ok(_) => debug!("created {:?}", path),
            Err(e) => println!("Error creating directory: {}", e),
        }
    }
    fn rmd(&mut self, path: PathBuf) {
        let path = self.cwd.join(path);
        // TODO: check path
        if invaild_path(&path) && remove_dir_all(&path) {
            self.send_answer(Answer::new(
                ResultCode::FileActOk,
                "Folder successufully removed",
            ));
        } else {
            self.send_answer(Answer::new(
                ResultCode::FileNotFound,
                "Couldn't remove folder",
            ));
        }
    }
    fn cwd(&mut self, dir: PathBuf) {
        let path = self.cwd.join(&dir);
        // TODO: check path invalid or exist
        if invaild_path(&path) {
            chdir(&path).expect("Change current directory fail");
            self.cwd = path;
        } else {
            self.send_answer(Answer::new(
                ResultCode::FileStatus,
                "No such file or directory",
            ));
        }
    }
    fn list(&mut self, path: Option<PathBuf>) {
        if self.data_conn.is_some() {
            let path = self.cwd.join(path.unwrap_or_default());
            // let directory = PathBuf::from(&path);
            self.send_answer(Answer::new(
                ResultCode::DataConnOpened,
                "Starting to list directory...",
            ));
            let mut out = Vec::new();
            if is_dir(&path.as_path()) {
                let dir = Dir::open(path.as_os_str(), OFlag::O_DIRECTORY, Mode::S_IXUSR).unwrap();
                dir.into_iter().for_each(|entry| {
                    add_file_info(entry.unwrap().file_name().to_str().unwrap(), &mut out);
                });
            } else {
                let path = path.as_os_str().to_str().unwrap();
                add_file_info(path, &mut out);
            }
            self.send_data(&out);
        } else {
            self.send_answer(Answer::new(
                ResultCode::ConnClose,
                "No opened data connection",
            ));
        }
    }
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
    fn pwd(&mut self) {
        let message = format!("{}", self.cwd.to_str().unwrap_or(""));
        if !message.is_empty() {
            let msg = format!("\"{}\" ", message);
            self.send_answer(Answer::new(ResultCode::CreatPath, msg.as_str()));
        } else {
            self.send_answer(Answer::new(
                ResultCode::FileNotFound,
                "No such file or directory",
            ));
        }
    }
    fn quit(&mut self) {
        if self.data_conn.is_some() {
            self.data_conn.clone().unwrap().lock().unwrap().shutdown();
        }
        self.cmd_conn.lock().unwrap().shutdown();
    }
    fn retr(&mut self, path: PathBuf) {
        if self.data_conn.is_some() {
            let path = self.cwd.join(path);
            // ugly module
            if is_regular(&path.as_path()) && self.is_admin {
                self.send_answer(Answer::new(
                    ResultCode::DataConnOpened,
                    "Starting to send file...",
                ));
                let fd =
                    open(path.as_os_str(), OFlag::O_RDWR, Mode::all()).expect("Can't open file");

                let mut len = 0;
                let size = get_file_size(&path);
                let mut buf = [0u8; 64 * 1024];
                while len < size {
                    match read(fd, &mut buf) {
                        Ok(n) => {
                            self.send_data(&buf[0..n]);
                            len += n;
                        }
                        Err(e) => debug!("Read file {:?} error {}", path, e),
                    }
                }
                info!("-> file transfer done!");
            } else {
            }
        } else {
            self.send_answer(Answer::new(
                ResultCode::ConnClose,
                "No opened data connection",
            ));
        }
    }

    fn stor(&mut self, path: PathBuf) {
        // local: hello remote: miniftp
        // 200 PORT command successful. Consider using PASV.
        // 150 Opening BINARY mode data connection for miniftp (21863760 bytes).
        // 226 Transfer complete.
        // 21863760 bytes received in 10.81 secs (1.9284 MB/s)
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
            if s == State::Finished || s == State::Closed {
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
            .send(format!("{} {}", answer.code as i32, answer.message).as_bytes())
    }

    fn send_data(&mut self, data: &[u8]) {
        if let Some(c) = self.data_conn.clone() {
            c.lock().unwrap().send(data)
        }
    }

    fn close_data_conn(&mut self) {
        if let Some(c) = &self.data_conn.clone() {
            c.lock().unwrap().shutdown();
            self.data_conn = None;
        }
    }
}
fn invaild_path(path: &Path) -> bool {
    for component in path.components() {
        if let Component::ParentDir = component {
            return true;
        }
    }
    false
}

pub fn is_dir(path: &Path) -> bool {
    let dir = path.as_os_str();
    match lstat(dir) {
        Ok(stat) => SFlag::S_IFDIR.bits() & stat.st_mode == SFlag::S_IFDIR.bits(),
        Err(e) => {
            warn!("Can't get file stat, {}", e);
            false
        }
    }
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

pub fn add_file_info(path: &str, out: &mut Vec<u8>) {
    // drwxr-xr-x 19 root root 646 Apr  3 12:14 ..
    // drwxr-xr-x  8 root root 272 Mar 29 20:33 handler
    // -rw-r--r--  1 root root 168 Mar 28 17:49 lib.rs
    let extra = if true { "/" } else { "" };
    let is_dir = if true { "d" } else { "-" };

    let stat = lstat(path).unwrap();

    // {mouth} {day} {hour}:{min}
    let naive = NaiveDateTime::from_timestamp(stat.st_ctime, 0);
    let datetime: DateTime<Utc> = DateTime::from_utc(naive, Utc);
    let time = datetime.format("%b %d %X").to_string();

    let size = stat.st_size;
    let rights = permissions(stat.st_mode);
    let links = stat.st_nlink;
    let owner = stat.st_uid;
    let group = stat.st_gid;

    let file_str = format!(
        "{is_dir}{rights} {links} {owner} {group} {size}  {time} {path}{extra}\r\n",
        is_dir = is_dir,
        rights = rights,
        links = links,
        owner = owner,
        group = group,
        size = size,
        time = time,
        path = path,
        extra = extra,
    );
    out.extend(file_str.as_bytes());
    println!("==> {:?}", file_str);
}

pub fn remove_dir_all(path: &Path) -> bool {
    if !is_dir(path) {
        return false;
    }
    let dir = Dir::open(path, OFlag::O_DIRECTORY, Mode::S_IXUSR).unwrap();
    for entry in dir.into_iter() {
        let entry = entry.unwrap();
        let path = Path::new(entry.file_name().to_str().unwrap());
        let file_type = entry.file_type().unwrap();
        if file_type == Type::Directory {
            remove_dir_all(path);
        } else if file_type == Type::Symlink {
            unlink(path).unwrap();
        } else if file_type == Type::File {
            // remove(path);
        }
    }
    // TODO: how to delete file
    // unsafe {
    //     rmdir(path.as_os_str() as *const c_char);
    // }
    true
}
