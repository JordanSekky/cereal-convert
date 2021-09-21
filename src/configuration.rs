extern crate serde;
use serde::Deserialize;
use std::fs::File;
use std::io::prelude::*;

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct Configuration {
    #[serde(default)]
    pub royalroad: RoyalRoadConfiguration,
}

#[derive(Deserialize, Debug)]
pub struct RoyalRoadConfiguration {
    pub ids: Vec<u32>,
}

impl Default for RoyalRoadConfiguration {
    fn default() -> Self {
        RoyalRoadConfiguration { ids: vec![] }
    }
}

impl Configuration {
    pub fn from_config_file() -> Configuration {
        let mut config = String::new();
        File::open("config.toml")
            .expect("Configuration file doesn't exist.")
            .read_to_string(&mut config)
            .expect("Failed to read bytes from configuration file.");
        return toml::from_str(&config).expect("Failed to convert toml to struct.");
    }
}
