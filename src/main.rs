extern crate omd_transfer;

use omd_transfer::*;

fn main() {
  let mut transfer = Transfer::new("foo/");
  transfer.refresh_items().expect("Failed to list items on camera");
  transfer.download_new().expect("Download failed");
}
