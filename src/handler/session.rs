use crate::server::connection::{ConnRef, Connection};
use mio::Event;
use nix::sys::socket::SockAddr;
use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;

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
pub struct Context {
    address: SockAddr,
    cur_path: Box<Path>,
    data_conn: ConnRef,
    cmd_conn: ConnRef,
    mode: Mode,
    data_type: DataType,
    breakpoint: usize,
    filename: String,
}

pub struct Session {
    contexts: HashMap<i32, Rc<Context>>,
}

impl Session {
    pub fn new() -> Self {
        // let path = Path::new("./foo/bar.txt");
        Session {
            contexts: HashMap::new(),
        }
    }
    pub fn get_context(&self, conn: ConnRef) -> Rc<Context> {
        let fd = conn.lock().unwrap().get_fd();
        self.contexts[&fd]
    }
    pub fn handle_command(&self, cmd: String) {
        // const std::map<std::string, std::function<void(std::string)>> command_map{
        //     // Access control commands
        //     {"USER", std::bind(&FtpSession::handleFtpCommandUSER, this,
        //                        std::placeholders::_1)},
        //     {"PASS", std::bind(&FtpSession::handleFtpCommandPASS, this,
        //                        std::placeholders::_1)},
        //     {"ACCT", std::bind(&FtpSession::handleFtpCommandACCT, this,
        //                        std::placeholders::_1)},
        //     {"CWD", std::bind(&FtpSession::handleFtpCommandCWD, this,
        //                       std::placeholders::_1)},
        //     {"CDUP", std::bind(&FtpSession::handleFtpCommandCDUP, this,
        //                        std::placeholders::_1)},
        //     {"REIN", std::bind(&FtpSession::handleFtpCommandREIN, this,
        //                        std::placeholders::_1)},
        //     {"QUIT", std::bind(&FtpSession::handleFtpCommandQUIT, this,
        //                        std::placeholders::_1)},

        //     // Transfer parameter commands
        //     {"PORT", std::bind(&FtpSession::handleFtpCommandPORT, this,
        //                        std::placeholders::_1)},
        //     {"PASV", std::bind(&FtpSession::handleFtpCommandPASV, this,
        //                        std::placeholders::_1)},
        //     {"TYPE", std::bind(&FtpSession::handleFtpCommandTYPE, this,
        //                        std::placeholders::_1)},
        //     {"STRU", std::bind(&FtpSession::handleFtpCommandSTRU, this,
        //                        std::placeholders::_1)},
        //     {"MODE", std::bind(&FtpSession::handleFtpCommandMODE, this,
        //                        std::placeholders::_1)},

        //     // Ftp service commands
        //     {"RETR", std::bind(&FtpSession::handleFtpCommandRETR, this,
        //                        std::placeholders::_1)},
        //     {"STOR", std::bind(&FtpSession::handleFtpCommandSTOR, this,
        //                        std::placeholders::_1)},
        //     {"STOU", std::bind(&FtpSession::handleFtpCommandSTOU, this,
        //                        std::placeholders::_1)},
        //     {"APPE", std::bind(&FtpSession::handleFtpCommandAPPE, this,
        //                        std::placeholders::_1)},
        //     {"ALLO", std::bind(&FtpSession::handleFtpCommandALLO, this,
        //                        std::placeholders::_1)},
        //     {"REST", std::bind(&FtpSession::handleFtpCommandREST, this,
        //                        std::placeholders::_1)},
        //     {"RNFR", std::bind(&FtpSession::handleFtpCommandRNFR, this,
        //                        std::placeholders::_1)},
        //     {"RNTO", std::bind(&FtpSession::handleFtpCommandRNTO, this,
        //                        std::placeholders::_1)},
        //     {"ABOR", std::bind(&FtpSession::handleFtpCommandABOR, this,
        //                        std::placeholders::_1)},
        //     {"DELE", std::bind(&FtpSession::handleFtpCommandDELE, this,
        //                        std::placeholders::_1)},
        //     {"RMD", std::bind(&FtpSession::handleFtpCommandRMD, this,
        //                       std::placeholders::_1)},
        //     {"MKD", std::bind(&FtpSession::handleFtpCommandMKD, this,
        //                       std::placeholders::_1)},
        //     {"PWD", std::bind(&FtpSession::handleFtpCommandPWD, this,
        //                       std::placeholders::_1)},
        //     {"LIST", std::bind(&FtpSession::handleFtpCommandLIST, this,
        //                        std::placeholders::_1)},
        //     {"NLST", std::bind(&FtpSession::handleFtpCommandNLST, this,
        //                        std::placeholders::_1)},
        //     {"SITE", std::bind(&FtpSession::handleFtpCommandSITE, this,
        //                        std::placeholders::_1)},
        //     {"SYST", std::bind(&FtpSession::handleFtpCommandSYST, this,
        //                        std::placeholders::_1)},
        //     {"STAT", std::bind(&FtpSession::handleFtpCommandSTAT, this,
        //                        std::placeholders::_1)},
        //     {"HELP", std::bind(&FtpSession::handleFtpCommandHELP, this,
        //                        std::placeholders::_1)},
        //     {"NOOP", std::bind(&FtpSession::handleFtpCommandNOOP, this,
        //                        std::placeholders::_1)},

        //     // Modern FTP Commands
        //     {"FEAT", std::bind(&FtpSession::handleFtpCommandFEAT, this,
        //                        std::placeholders::_1)},
        //     {"OPTS", std::bind(&FtpSession::handleFtpCommandOPTS, this,
        //                        std::placeholders::_1)},
        //     {"SIZE", std::bind(&FtpSession::handleFtpCommandSIZE, this,
        //                        std::placeholders::_1)},
        // };
    }
    d
}
