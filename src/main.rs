use clap::Parser;
use log::LevelFilter;
use miniftp::{self, is_root_user, local_client, set_log_level};

/// Search for a pattern in a file and display the lines that contain it.
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    /// The pattern to look for
    #[clap(short, long, default_value = "server")]
    pattern: String,
    /// The path to the file to read
    #[clap(parse(from_os_str), short, long, default_value = "")]
    config: std::path::PathBuf,
}

fn main() {
    set_log_level(LevelFilter::Debug);
    let args = Cli::parse();
    if args.pattern.eq("client") {
        let mut client = local_client::LocalClient::new();
        println!("starting minFTP shell");
        client.shell_loop();
    } else if args.pattern.eq("server") {
        // if !is_root_user() {
        //     println!("minftp: must be started as root user.");
        //     return;
        // }
        let config = args.config.canonicalize().unwrap();
        println!("config: {:?}", config);
        miniftp::run_server(&config);
    }
}
