use nix::sys::socket::InetAddr;
use super::socket::{Socket, inet_addr};
pub struct Acceptor {
    accept_socket: Socket,
    listening: bool,
}

impl Acceptor {
    pub fn new(addr: &str) -> Self {
        // let sock_addr = inet_addr(addr);
        let acceptor_sock = Socket::bind(addr);
        acceptor_sock.set_reuse_addr(true);
        let v:Vec<&str> = addr.split(|c: char| c == ':').collect();
        acceptor_sock.set_reuse_port(v[1].parse::<u16>().unwrap());
        Acceptor {
            accept_socket: acceptor_sock,
            listening: true,
        }
    }
    pub fn listening(&self) -> bool {
        self.listening
    }
    pub fn accept(listen_fd: i32) -> Socket {
        let mut sock = Socket::accept(listen_fd);
        sock.set_no_delay(true);
        sock.set_keep_alive(true);
        sock
    }
}
