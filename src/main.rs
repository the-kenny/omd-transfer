extern crate omd_transfer;
extern crate env_logger;

use omd_transfer::*;
use std::env;

fn main() {
  env_logger::init().unwrap();

  let config_file = env::var("OMD_TRANSFER_CONFIG")
    .unwrap_or("config.toml".into());

  let config = Config::from_file(&config_file);
  
  OrderTransfer::from_config(&config).map(|transfer| {
    execute_transfer(transfer, &config).unwrap();
  });
  
  IncrementalTransfer::from_config(&config).map(|transfer| {
    execute_transfer(transfer, &config).unwrap();
  });
}
