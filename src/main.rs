extern crate omd_transfer;
extern crate env_logger;

use omd_transfer::*;

fn main() {
  env_logger::init().unwrap();

  let config = Config {
    download_dir: Some("foo/".into()),
    transfer_order_dir: Some("transfer_order/".into()),
    error_strategy: ErrorStrategy::Abort,
  };

  OrderTransfer::from_config(&config).map(|transfer| {
    execute_transfer(transfer, config.error_strategy).unwrap();
  });
  
  IncrementalTransfer::from_config(&config).map(|transfer| {
    execute_transfer(transfer, config.error_strategy).unwrap();
  });
}
