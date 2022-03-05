use chrono::Local;
use env_logger::Builder;
use log::LevelFilter;
use miniftp;
use std::env;
use std::io::Write;
fn main() {
    miniftp::signal_ignore();
    Builder::new()
        .format(|buf, record| {
            writeln!(
                buf,
                "{} {} {}:{} - {}",
                Local::now().format("%Y-%m-%d %H:%M:%S"),
                record.level(),
                record.file_static().unwrap(),
                record.line().unwrap(),
                record.args(),
            )
        })
        .filter(None, LevelFilter::Debug)
        .init();
    if let Some(ref opt) = env::args().nth(1) {
        if opt == "-c" {
            // let mut client = LocalClient;
            println!("starting minFTP shell");
            // client.shell_loop();
        } else {
            println!("invalid option {}, only support `-c`", opt);
        }
    } else {
        miniftp::run_server();
    }
}
