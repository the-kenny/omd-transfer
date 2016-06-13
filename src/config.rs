use std::path::{Path,PathBuf};
use std::io::{Read};
use std::fs::File;

use toml;

#[derive(PartialEq, Debug, Clone, Copy)]
pub enum ErrorStrategy {
  Abort,
  Continue,
}

impl ErrorStrategy {
  fn from_str(v: &str) -> Option<Self> {
    match v {
      "abort"    => Some(ErrorStrategy::Abort),
      "continue" => Some(ErrorStrategy::Continue),
      _ => None
    }
  }
}

#[derive(PartialEq, Debug, Clone, Copy)]
pub enum OverwriteStrategy {
  Overwrite,
  Skip,
}

impl OverwriteStrategy {
  fn from_str(v: &str) -> Option<Self> {
    match v {
      "overwrite" => Some(OverwriteStrategy::Overwrite),
      "skip"      => Some(OverwriteStrategy::Skip),
      _           => None
    }
  }
}

#[derive(Clone, Debug)]
pub struct WifiConfig {
  pub interface: String,
  pub ssid: String,
}

#[derive(Clone, Debug)]
pub struct Config {
  pub download_dir: Option<PathBuf>,
  pub transfer_order_dir: Option<PathBuf>,

  pub error_strategy: ErrorStrategy,
  pub overwrite_strategy: OverwriteStrategy,

  pub power_off: bool,

  pub wifi: Option<WifiConfig>
}

impl Config {
  pub fn from_file<P: AsRef<Path>>(file: P) -> Self {
    info!("Loading config from {}", file.as_ref().display());

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

    let overwrite_strategy = conf.lookup("overwrite_strategy")
      .expect("`overwrite_strategy` not found in config file")
      .as_str()
      .and_then(OverwriteStrategy::from_str)
      .expect("Invalid overwrite_strategy");


    let incremental_dir = conf.lookup("incremental.download_directory")
      .and_then(toml::Value::as_str)
      .map(PathBuf::from);

    let transfer_order_dir = conf.lookup("transfer_order.download_directory")
      .and_then(toml::Value::as_str)
      .map(PathBuf::from);

    let power_off = conf.lookup("power_off")
      .and_then(toml::Value::as_bool)
      .expect("`power_off` not found in config file");

    
    let wifi = conf.lookup("wifi.interface")
      .and_then(toml::Value::as_str)
      .and_then(|i| {
        conf.lookup("wifi.ssid")
          .and_then(toml::Value::as_str)
          .map(|s| WifiConfig {
            ssid: s.into(),
            interface: i.into(),
          })
      });

    Config {
      download_dir: incremental_dir,
      transfer_order_dir: transfer_order_dir,
      error_strategy: error_strategy,
      overwrite_strategy: overwrite_strategy,

      power_off: power_off,

      wifi: wifi
    }
  }

  pub fn template() -> &'static str {
    include_str!("../config.template.toml")
  }
}
