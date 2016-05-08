use std::path::{PathBuf};

#[derive(PartialEq,Debug, Clone, Copy)]
pub enum ErrorStrategy {
  Abort,
  Continue,
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
  
  // pub fn from_file<P: AsRef<Path>>(file: P) -> io::Result<Self> {
  //   unimplemented!()
  // }
}
