use clap::Parser;
use log::LevelFilter;
use miniftp::{self, is_root_user, local_client, set_log_level};

/// Search for a pattern in a file and display the lines that contain it.
#[derive(Parser, Debug)]
struct Cli {
    /// The pattern to look for
    pattern: String,
    /// The path to the file to read
    #[clap(parse(from_os_str))]
    config: std::path::PathBuf,
}

fn main() {
    set_log_level(LevelFilter::Debug);
    let args = Cli::parse();
    if args.pattern.eq("c") {
        let mut client = local_client::LocalClient::new();
        println!("starting minFTP shell");
        client.shell_loop();
    } else {
        if !is_root_user() {
            println!("TinyFTPD: must be started as root user.");
            return;
        }
        miniftp::run_server(&args.config);
    }
}
