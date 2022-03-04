use crate::utils::config::{self, Config};
use std::{fmt::Error, io::stdin};
#[derive(Debug)]
pub struct LocalClient;
impl LocalClient {
    pub fn shell_loop(&mut self) {
        let config = Config::from_cwd_config();
        let mut line = String::new();

        loop {
            print!("FTP> ");
            match stdin().read_line(&mut line) {
                Ok(n) => {
                    line.pop();
                    if n == 0 {
                        continue;
                    }
                    // cmd_handler
                    // let Ok(out) = cmd_handler(cmd)
                    line.clear();
                }
                Err(error) => println!("error: {}", error),
            }
        }
    }
}
