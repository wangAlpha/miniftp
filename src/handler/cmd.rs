use super::error::{Error, Result};
use std::path::Path;
use std::path::PathBuf;
use std::str::{self, FromStr};
use std::result;

pub struct Answer {
    pub code: ResultCode,
    pub message: String,
}
impl Answer {
    pub fn new(code: ResultCode, message: &str) -> Self {
        Answer { code, message: message.to_string() }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Auth,
    Cwd(PathBuf),
    List(Option<PathBuf>),
    Mkd(PathBuf),
    NoOp,
    Port(u16),
    Pass(String),
    Pasv,
    Pwd,
    Quit,
    Retr(PathBuf),
    Rmd(PathBuf),
    Stor(PathBuf),
    Syst,
    Type(TransferType),
    CdUp(String),
    User(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum TransferType {
    ASCII,
    BINARY,
    Unknown,
}

impl From<u8> for TransferType{
    fn from(c: u8) -> TransferType{
        match c {
            b'A'=>TransferType::ASCII,
            b'I'=>TransferType::BINARY,
            _=>TransferType::Unknown,
        }
    }
}
impl AsRef<str> for Command {
    fn as_ref(&self) -> &str {
        match *self {
            Command::Auth => "AUTH",
            Command::Cwd(_) => "CWD",
            Command::Pass(_) => "PASS",
            Command::List(_) => "LIST",
            Command::Mkd(_) => "MKD",
            Command::NoOp => "NOOP",
            Command::Port(_) => "PORT",
            Command::Pasv => "PASV",
            Command::Pwd => "PWD",
            Command::Quit => "QUIT",
            Command::Retr(_) => "RETR",
            Command::Rmd(_) => "RMD",
            Command::Stor(_) => "STOR",
            Command::Syst => "SYST",
            Command::Type(_) => "TYPE",
            Command::CdUp(_) => "CDUP",
            Command::User(_) => "USER",
        }
    }
}

impl Command {
    pub fn new(input: Vec<u8>) -> Result<Self> {
        let mut iter = input.split(|&byte| byte == b' ');
        let mut command = iter
            .next()
            .ok_or_else(|| Error::Msg("empty command".to_string()))
            .unwrap()
            .to_vec();

        // to uppercase
        let command:Vec<u8> = command.iter().map(|&x| {
            if x >= 'a' as u8 && x <= 'z' as u8 {
                x -= 32
            }
            x
        }).collect();
        let data = iter
            .next()
            .ok_or_else(|| Error::Msg("no command parameter".to_string()));
        let command = match command.as_slice() {
            b"AUTH" => Command::Auth,
            b"CWD" => Command::Cwd(data.and_then(to_path_buf)?),
            b"LIST" => Command::List(data.and_then(to_path_buf)),
            b"PASS" => Command::Pass(data.and_then(to_path_buf)),
            b"PASV" => Command::Pasv,
            b"PORT" => extract_port(data?),
            b"PWD" => Command::Pwd,
            b"QUIT" => Command::Quit,
            b"RETR" => Command::Retr(data.and_then(to_path_buf)),
            b"STOR" => Command::Stor(data.and_then(to_path_buf)),
            b"SYST" => Command::Syst,
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
            b"USER" => Command::User(
                data.and_then(|bytes| Ok(Path::new(str::from_utf8(bytes)?).))?,
            ),
            b"CDUP" => Command::CdUp,
            b"MKD" => Command::Mkd(data.and_then(to_path_buf)),
            b"RMD" => Command::Rmd(
                data.and_then(|bytes| Ok(Path::new(str::from_utf8(bytes)?).to_path_buf()))?,
            ),
            b"NOOP" => Command::NoOp,
            s => Command::Unknown(str::from_utf8(s).unwrap_or("").to_owned()),
        };
        Ok(command)
    }
}

pub fn to_path_buf(data: &[u8]) -> PathBuf {
    Path::new(str::from_utf8(data).unwrap()).to_path_buf()
}

pub fn extract_port(data: &[u8]) -> Command {
    let addr = data
        .split(|&byte| byte == b',')
        .filter_map(|bytes| str::from_utf8(|string| u8::from_str(string).ok()))
        .collect::<Vec<u8>>();
    if addr.len() != 6 {
        return Err("Invalid address/port".into());
    }

    let port = (addr[4] as u16) << 8 | (addr[5] as u16);
    if port <= 1024 {
        return Err("Port can't be less than 1025".into());
    }
    Command::Port(port)
}

#[derive(Debug, Clone, Copy)]
pub enum ResultCode {
    Series = 100,
    RestartMakerReplay = 110,
    ServiceReady = 120,
    DataConnOpened = 125,
    FileStatusOk = 150,
    Ok = 200,
    //CmdNotImpl=202,
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
    NeedPsk = 331,
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
