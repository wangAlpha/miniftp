use super::socket::Socket;
pub struct Acceptor {
    accept_socket: Socket,
    listening: bool,
}

impl Acceptor {
    pub fn new(addr: &str) -> Self {
        Acceptor {
            accept_socket: Socket::bind(addr),
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
