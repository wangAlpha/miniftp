use std::fs::OpenOptions;
use std::io::Read;
use yaml_rust::YamlLoader;

#[derive(Debug, Clone)]
pub struct Config {
    config: yaml_rust::Yaml,
}

impl Config {
    pub fn new(s: &String) -> Config {
        let docs = YamlLoader::load_from_str(s).unwrap();
        Config {
            config: docs[0].to_owned(),
        }
    }
    pub fn from_cwd_config() -> Config {
        let cwd = "./config.yaml".to_string();
        Self::from_file(&cwd)
    }
    pub fn from_file(file: &String) -> Config {
        let mut f = OpenOptions::new().read(true).open(file).unwrap();
        let mut buf = String::new();
        f.read_to_string(&mut buf).unwrap();
        Self::new(&buf)
    }
    pub fn get_str(&self, path: &str) -> String {
        String::from(self.config[path].as_str().unwrap())
    }
    pub fn get_int(&self, path: &str) -> i64 {
        self.config[path].as_i64().unwrap()
    }
}
