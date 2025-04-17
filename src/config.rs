use std::cell::RefCell;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::PathBuf;
use colored::Colorize;
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Serialize, Deserialize)]
pub(crate) struct Config {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    #[serde(skip)]
    config_file_path: PathBuf,
}

const DEFAULT_BASE_URL: &str = "https://ark.cn-beijing.volces.com/api/v3";
const DEFAULT_MODEL: &str = "deepseek-r1-250120";
const DEFAULT_API_KEY: &str = "6f1797f8-b0d5-4a1e-9450-17ed67c0ad2f";

impl Config {
    pub fn new() -> Self {
        let mut config = Self {
            base_url: String::new(),
            api_key: String::new(),
            model: String::new(),
            config_file_path: PathBuf::new(),
        };

        config.get_default_config_file();
        config.load_config();
        config
    }

    fn get_default_config_file(&mut self) {
        let home_dir = dirs::home_dir().expect("Failed to get home directory");
        let mut config_dir = match std::env::consts::OS {
            "windows" => home_dir.join("AppData").join("Local").join("rag"),
            "linux" => home_dir.join(".config").join("rag"),
            os => {
                let default_path = home_dir.join(".config").join("rag");
                println!("{}", format!("Unsupported OS: {}, using default path: {:?}", os, default_path).yellow());
                default_path
            }
        };

        config_dir.push("rag.json");
        self.config_file_path = config_dir;
    }

    fn ensure_config_file_exists(&mut self) -> bool {
        std::fs::create_dir_all(self.config_file_path.parent().unwrap()).expect("Failed to create config dir");
        if !self.config_file_path.exists() {
            let config_file_path = self.config_file_path.as_path();
            File::create(config_file_path).expect("Failed to create config file");

            println!("{}", format!("Cannot to find config file, Using default config and creating: {:?}", config_file_path).red());
            println!("{}", format!("    base_url: {}", &DEFAULT_BASE_URL).yellow());
            println!("{}", format!("    model: {}", &DEFAULT_MODEL).yellow());
            println!("{}", format!("    api_key: {}", &DEFAULT_API_KEY).yellow());

            self.api_key = DEFAULT_API_KEY.to_string();
            self.model = DEFAULT_MODEL.to_string();
            self.base_url = DEFAULT_BASE_URL.to_string();
            self.save_config();

            return false;
        }
        true
    }

    pub fn save_config(&mut self) {
        let mut file = OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(self.config_file_path.as_path())
            .expect("Failed to open config file");
        let config_json = serde_yaml::to_string(self).expect("Failed to serialize config file");
        file.write_all(&config_json.into_bytes()).expect("Failed to write config file");
    }

    fn load_config(&mut self) {
        if self.ensure_config_file_exists() {
            let mut file = File::open(self.config_file_path.as_path()).expect("Failed to open config file");
            let mut config_string = String::new();

            file.read_to_string(&mut config_string).expect("Failed to read from config file");
            *self = serde_yaml::from_str(config_string.as_str()).expect("Failed to deserialize config");
        }
    }
}
//
// lazy_static! {
//     // Because we may need to modify config.
//     static ref CONFIG: RefCell<Config> = {
//         let mut config = Config::new();
//         config.load_config();
//         RefCell::new(config)
//     };
// }