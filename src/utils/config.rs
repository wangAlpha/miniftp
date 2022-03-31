use std::io::Read;
use std::{fs::OpenOptions, str::FromStr};
// use yaml_rust::YamlLoader;

pub const DEFAULT_PORT: u16 = 8089;
#[derive(Debug, Clone)]
pub struct Config {
    pub server_port: Option<u16>,
    pub server_addr: Option<String>,
    pub admin: Option<User>,
    pub users: Vec<User>,
    // config: yaml_rust::Yaml,
}

#[derive(Debug, Clone)]
pub struct User {
    pub name: String,
    pub password: String,
}

impl User {
    pub fn new() -> Self {
        User {
            name: String::from_str("root").unwrap(),
            password: String::from_str("password").unwrap(),
        }
    }
}

impl Config {
    pub fn new(path: &String) -> Config {
        Config {
            server_port: Some(8089),
            server_addr: Some(String::from_str("0.0.0.0").unwrap()),
            admin: Some(User::new()),
            users: Vec::new(),
        }
    }
    // pub fn from_cwd_config() -> Config {
    //     let cwd = "./config.yaml".to_string();
    //     Self::from_file(&cwd)
    // }
    pub fn from_file(file: &String) -> Option<String> {
        let mut f = OpenOptions::new().read(true).open(file).ok()?;
        let mut content = String::new();
        f.read_to_string(&mut content).ok()?;
        Some(content)
    }
    // pub fn get_str(&self, path: &str) -> String {
    //     String::from(self.config[path].as_str().unwrap())
    // }
    // pub fn get_int(&self, path: &str) -> i64 {
    //     self.config[path].as_i64().unwrap()
    // }
}
