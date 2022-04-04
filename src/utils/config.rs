use log::debug;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::str::FromStr;

pub const DEFAULT_PORT: u16 = 8089;
pub const DEFAULT_CONF_FILE: &'static str = "config.yaml";
pub type User = (String, String);
pub type Users = HashMap<String, String>;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Config {
    pub server_addr: Option<String>,
    pub server_port: Option<u16>,
    pub pasv_enable: bool,
    pub pasv_port: Vec<u16>,
    pub max_clients: usize,
    pub ssl_enable: bool,
    pub rsa_cert_file: Option<String>,
    pub rsa_private_key_file: Option<String>,
    pub admin: Option<User>,
    pub users: Users,
}

pub fn get_content(path: &str) -> Option<String> {
    let mut file = File::open(path).unwrap();
    let mut content = String::new();
    file.read_to_string(&mut content).ok()?;
    Some(content)
}

impl Config {
    pub fn new(path: &str) -> Config {
        if let Some(content) = get_content(path) {
            serde_yaml::from_str::<Config>(content.as_str()).unwrap()
        } else {
            debug!(
                "No config file found so creating new one in {}",
                DEFAULT_CONF_FILE
            );
            let config = Config {
                server_addr: Some(String::from_str("0.0.0.0").unwrap()),
                server_port: Some(8089),
                pasv_enable: true,
                pasv_port: vec![2222, 2222],
                max_clients: 0,
                ssl_enable: false,
                rsa_cert_file: None,
                rsa_private_key_file: None,
                admin: Some((String::new(), String::new())),
                users: HashMap::from([("anonymous".to_string(), "".to_string())]),
            };

            let content = serde_yaml::to_string(&config).expect("serialization failed");
            let file = File::create(DEFAULT_CONF_FILE).expect("couldn't create file...");
            debug!("{}", content);
            config
        }
    }
}
