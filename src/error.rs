use std;
use std::io;

use hyper;

#[derive(Debug)]
pub enum Error {
  Http(hyper::Error),
  Io(io::Error),
  ProtocolError,
}

pub type Result<T> = std::result::Result<T,Error>;

impl From<io::Error> for Error {
  fn from(err: io::Error) -> Self {
      Error::Io(err)
  }
}

impl From<hyper::Error> for Error {
  fn from(err: hyper::Error) -> Self {
      Error::Http(err)
  }
}

