extern crate omd_transfer;
extern crate env_logger;

use omd_transfer::*;

fn main() {
  env_logger::init().unwrap();
  
  let mut transfer = Transfer::new("foo/");
  transfer.download_transfer_order().expect("Failed to download transfer order");
  transfer.refresh_items().expect("Failed to list items on camera");
  transfer.download_new().expect("Download failed");
}
