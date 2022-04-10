use super::error::{Error, Result};
use enum_primitive_derive::Primitive;
use nix::libc::write;
use num_traits::FromPrimitive;
use std::fmt;
use std::path::PathBuf;
use std::str::{self, FromStr};

#[derive(Debug, Clone, PartialEq)]
pub struct Answer {
    pub code: ResultCode,
    pub message: String,
}

impl Answer {
    pub fn new(code: ResultCode, message: &str) -> Self {
        Answer {
            code,
            message: message.to_string(),
        }
    }
    pub fn from(buf: &str) -> Option<Self> {
        let s = buf.to_string();
        if let Some(index) = s.find(' ') {
            if index < 7 {
                let (code, message) = s.split_at(index);
                println!("code:{:?},msg:{:?}", code, message);
                let code = ResultCode::from_i32(code.parse::<i32>().unwrap()).unwrap();
                return Some(Answer::new(code, &message[1..]));
            }
        }
        None
    }
}

impl fmt::Display for Answer {
    // This trait requires `fmt` with this exact signature.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.message.is_empty() {
            write!(f, "{} \\r\\n", self.code as i32)
        } else {
            write!(f, "{} {} \\r\\n", self.code as i32, self.message)
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Acct,
    Cwd(PathBuf),
    List(Option<PathBuf>),
    NList(Option<PathBuf>),
    Mkd(PathBuf),
    Port(u16),
    Pass(String),
    Rest(String),
    NoOp,
    Pasv,
    Pwd,
    Quit,
    Abort,
    Syst,
    CdUp,
    Retr(PathBuf),
    Rmd(PathBuf),
    Rnfr(PathBuf),
    Rnto(PathBuf),
    Stor(PathBuf),
    Stat(PathBuf),
    Type(TransferType),
    User(String),
    Help(String),
    Unknown(String),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TransferType {
    ASCII,
    BINARY,
    Unknown,
}

impl From<u8> for TransferType {
    fn from(c: u8) -> TransferType {
        match c {
            b'A' => TransferType::ASCII,
            b'I' => TransferType::BINARY,
            _ => TransferType::Unknown,
        }
    }
}

impl fmt::Display for TransferType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            TransferType::ASCII => write!(f, "ASCII"),
            TransferType::BINARY => write!(f, "BINARY"),
            TransferType::Unknown => write!(f, "UNKNOWN"),
        }
    }
}

impl AsRef<str> for Command {
    fn as_ref(&self) -> &str {
        match *self {
            Command::Acct => "ACCT",
            Command::Cwd(_) => "CWD",
            Command::Pass(_) => "PASS",
            Command::List(_) => "LIST",
            Command::NList(_) => "NLIST",
            Command::Mkd(_) => "MKD",
            Command::NoOp => "NOOP",
            Command::Port(_) => "PORT",
            Command::Pasv => "PASV",
            Command::Pwd => "PWD",
            Command::Quit => "QUIT",
            Command::Abort => "ABORT",
            Command::Rest(_) => "REST",
            Command::Retr(_) => "RETR",
            Command::Rmd(_) => "RMD",
            Command::Rnfr(_) => "RNFR",
            Command::Rnto(_) => "RNTO",
            Command::Stor(_) => "STOR",
            Command::Syst => "SYST",
            Command::Type(_) => "TYPE",
            Command::Help(_) => "HELP",
            Command::Stat(_) => "STAT",
            Command::CdUp => "CDUP",
            Command::User(_) => "USER",
            Command::Unknown(_) => "UNKN",
        }
    }
}

impl Command {
    pub fn new(input: Vec<u8>) -> Result<Self> {
        let mut iter = input.split(|&byte| byte == b' ');
        let command = iter
            .next()
            .ok_or_else(|| Error::Msg("empty command".to_string()))
            .unwrap()
            .to_vec();
        // 先移除\r\
        // debug!("command: {}", String::from_utf8(command.clone()).unwrap());
        // to uppercase
        let command = String::from_utf8_lossy(&command).to_ascii_uppercase();
        let data = iter
            .next()
            .ok_or_else(|| Error::Msg("no command parameter".to_string()));
        // let d = String::from_utf8_lossy(data?).to_string();
        let command = match command.as_bytes() {
            b"Acct" => Command::Acct,
            b"PASV" => Command::Pasv,
            b"PWD" => Command::Pwd,
            b"QUIT" => Command::Quit,
            b"ABORT" => Command::Abort,
            b"SYST" => Command::Syst,
            b"CDUP" => Command::CdUp,
            b"NOOP" => Command::NoOp,
            b"REST" => Command::Rest(String::from_utf8_lossy(data?).to_string()),
            b"CWD" => Command::Cwd(PathBuf::from(String::from_utf8_lossy(data?).to_string())),
            b"PASS" => Command::Pass(String::from_utf8_lossy(data?).to_string()),
            b"RETR" => Command::Retr(PathBuf::from(String::from_utf8_lossy(data?).to_string())),
            b"RNFR" => Command::Rnfr(PathBuf::from(String::from_utf8_lossy(data?).to_string())),
            b"RNTO" => Command::Rnto(PathBuf::from(String::from_utf8_lossy(data?).to_string())),
            b"STOR" => Command::Stor(PathBuf::from(String::from_utf8_lossy(data?).to_string())),
            b"STAT" => Command::Stat(PathBuf::from(String::from_utf8_lossy(data?).to_string())),
            b"LIST" => Command::List(if data.is_ok() {
                Some(PathBuf::from(String::from_utf8_lossy(data?).to_string()))
            } else {
                Some(PathBuf::from_str(".").unwrap())
            }),
            b"NLIST" => Command::NList(if data.is_ok() {
                Some(PathBuf::from(String::from_utf8_lossy(data?).to_string()))
            } else {
                Some(PathBuf::from_str(".").unwrap())
            }),
            b"PORT" => extract_port(data?).unwrap(),
            b"TYPE" => {
                let error = Err("command not implemented for that parameter".into());
                let data = data?;
                if data.is_empty() {
                    return error;
                }
                match TransferType::from(data[0]) {
                    TransferType::Unknown => return error,
                    typ => Command::Type(typ),
                }
            }
            b"USER" => Command::User(String::from_utf8_lossy(data?).to_string()),
            b"HELP" => Command::Help(String::from_utf8_lossy(data?).to_string()),
            b"MKD" => Command::Mkd(PathBuf::from(String::from_utf8_lossy(data?).to_string())),
            b"RMD" => Command::Rmd(PathBuf::from(String::from_utf8_lossy(data?).to_string())),
            s => Command::Unknown(str::from_utf8(s).unwrap_or("").to_owned()),
        };
        Ok(command)
    }
}

pub fn extract_port(data: &[u8]) -> Result<Command> {
    let addr = data
        .split(|&byte| byte == b',')
        .filter_map(|bytes| {
            str::from_utf8(bytes)
                .ok()
                .and_then(|s| u8::from_str(s).ok())
        })
        .collect::<Vec<u8>>();
    // debug!("addr: {}", String::from_utf8(addr).unwrap());
    if addr.len() != 6 {
        return Err("Invalid address/port".into());
    }

    let port = (addr[4] as u16) << 8 | (addr[5] as u16);
    if port <= 1024 {
        return Err("Port can't be less than 1025".into());
    }
    Ok(Command::Port(port))
}

#[derive(Debug, Clone, Copy, PartialEq, Primitive)]
pub enum ResultCode {
    Series = 100,
    RestartMakerReplay = 110,
    ServiceReady = 120,
    DataConnOpened = 125,
    FileStatusOk = 150,
    Ok = 200,
    SysStatus = 211,
    DirStatus = 212,
    FileStatus = 213,
    HelpMsg = 214,
    NameSysType = 215,
    ServiceReadyForUsr = 220,
    ServiceCloseCtlCon = 221,
    DataConnOpen = 225,
    CloseDataClose = 226,
    PassMode = 227,
    LongPassMode = 228,
    EntendedPassMode = 229,
    Login = 230,
    Logout = 231,
    LogoutCmd = 232,
    FileActOk = 250,
    CreatPath = 257,
    NeedPsw = 331,
    NeedAccount = 332,
    FileActionPending = 350,
    ServiceNotAvail = 421,
    DataConnFail = 425,
    ConnClose = 426,
    FileBusy = 450,
    LocalErrr = 451,
    NotEnoughSpace = 452,
    SyntaxErr = 500,
    CmdNotImpl = 502,
    BadCmdSeq = 503,
    CmdNotCmplParam = 504,
    NotLogin = 530,
    NeedAccountStoringFiles = 532,
    FileNotFound = 550,
    PageTypeUnknown = 551,
    ExceededStorageAlloc = 552,
    FileNameNotAllow = 553,
}
