use std::path::{Path,PathBuf};
use std::io::{Read};
use std::fs::File;

use toml;

#[derive(PartialEq,Debug, Clone, Copy)]
pub enum ErrorStrategy {
  Abort,
  Continue,
}

impl ErrorStrategy {
  fn from_str(v: &str) -> Option<ErrorStrategy> {
    match v {
      "abort"    => Some(ErrorStrategy::Abort),
      "continue" => Some(ErrorStrategy::Continue),
      _ => None
    }
  }
}

#[derive(Clone, Debug)]
pub struct Config {
  pub download_dir: Option<PathBuf>,
  pub transfer_order_dir: Option<PathBuf>,

  pub error_strategy: ErrorStrategy,
}

impl Config {
  pub fn new() -> Self {
    Config {
      download_dir: Some("downloaded/".into()),
      transfer_order_dir: Some("transfer_order/".into()),

      error_strategy: ErrorStrategy::Abort,
    }
  }

  pub fn from_file<P: AsRef<Path>>(file: P) -> Self {
    let conf: toml::Value = {
      let mut buf = String::new();
      let mut file = File::open(file).expect("Failed to parse config!");
      file.read_to_string(&mut buf).expect("Failed to parse config!");

      buf.parse().expect("Failed to parse config!")
    };

    let error_strategy = conf.lookup("error_strategy")
      .expect("`error_strategy` not found in config file")
      .as_str()
      .and_then(ErrorStrategy::from_str)
      .expect("Invalid error_strategy");

    let incremental_dir = conf.lookup("incremental.download_directory")
      .map(toml::Value::to_string)
      .map(PathBuf::from);

    let transfer_order_dir = conf.lookup("transfer_order.download_directory")
      .map(toml::Value::to_string)
      .map(PathBuf::from);

    Config {
      download_dir: incremental_dir,
      transfer_order_dir: transfer_order_dir,
      error_strategy: error_strategy
    }
  }
}
