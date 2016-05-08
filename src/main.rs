extern crate omd_transfer;
extern crate env_logger;

use omd_transfer::*;

fn main() {
  env_logger::init().unwrap();

  let config = Config {
    download_dir: Some("foo/".into()),
    transfer_order_dir: None,
    error_strategy: ErrorStrategy::Abort,
  };
  
  let transfer = Transfer::from_config(&config).unwrap();
  transfer.download_new().expect("Download failed");
}
