use miniftp;
use std::env;

fn main() {
    if let Some(ref opt) = env::args().nth(1) {
        if opt == "-c" {
            let mut client = blastoise::LocalClient;
            println!("starting minFTP shell");
            client.shell_loop();
        } else {
            println!("invalid option {}, only support `-c`", opt);
        }
    } else {
        miniftp::run_server();
    }
}
