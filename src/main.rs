use log::{info, LevelFilter};
use miniftp::{self, is_root_user, local_client, set_log_level};
use std::env;

fn main() {
    set_log_level(LevelFilter::Debug);
    if let Some(ref opt) = env::args().nth(1) {
        if !is_root_user() {
            info!("TinyFTPD: must be started as root user.");
            return;
        }
        if opt == "-c" {
            let mut client = local_client::LocalClient::new();
            println!("starting minFTP shell");
            client.shell_loop();
        } else {
            println!("invalid option {}, only support `-c`", opt);
        }
    } else {
        miniftp::run_server();
    }
}
