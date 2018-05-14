use std::{fs, io};
use std::collections::LinkedList;
use std::fs::File;
use std::io::{Read,Write};
use std::path::{Path,PathBuf};

use error::{Error,Result};
use config::*;

use chrono::{NaiveDate,NaiveDateTime};
use hyper::Client;
use hyper::status::StatusCode;
use regex::Regex;

const BASE_URL: &'static str = "http://192.168.0.10/";

#[derive(Debug, PartialEq, Eq)]
pub struct TransferItem {
  pub parent: String,
  pub filename: String,
  pub file_size: u64,
  pub date: NaiveDateTime,
}

fn parse_fat_datetime(date: u16, time: u16) -> NaiveDateTime {
  let day     =  date       & 0b00011111;
  let month   =  date >> 5  & 0b00001111;
  let year    = (date >> 9  & 0b01111111) + 1980;
  let seconds = (time       & 0b00011111) * 2;
  let minutes =  time >> 5  & 0b00111111;
  let hours   =  time >> 11 & 0b00011111;
  assert!(seconds < 60);

  NaiveDate::from_ymd(year as i32, month as u32, day as u32)
    .and_hms(hours as u32, minutes as u32, seconds as u32)
}

#[test]
fn test_parse_fat_datetime() {
  let d = NaiveDate::from_ymd(2016, 4, 27).and_hms(19, 39, 10);
  assert_eq!(parse_fat_datetime(18587,40165), d);
}

impl TransferItem {
  pub fn from_row(row: &str) -> Self {
    let row: Vec<&str> = row.split(",").collect();
    let size = u64::from_str_radix(row[2], 10).expect("Invalid size");
    let _ = row[3];
    let fat_date = u16::from_str_radix(row[4], 10).expect("Invalid date");
    let fat_time = u16::from_str_radix(row[5], 10).expect("Invalid time");
    let date = parse_fat_datetime(fat_date,fat_time);

    TransferItem {
      parent: row[0].to_string(),
      filename: row[1].to_string(),
      file_size: size,
      date: date,
    }
  }

  pub fn is_directory(&self) -> bool {
    lazy_static! {
      static ref RE: Regex = Regex::new(r"^\d{3}\w{5}$").unwrap();
    }
    RE.is_match(&self.filename)
  }

  pub fn path(&self) -> String {
    format!("{}/{}", self.parent, self.filename)
  }

  pub fn download<P: AsRef<Path>>(&self,
                                  client: &Client,
                                  target: &P,
                                  strategy: OverwriteStrategy) -> Result<()> {
    let url = format!("{}{}", BASE_URL, self.path());
    debug!("Fetching {}", url);
    let mut res = try!(client.get(&url).send());
    assert_eq!(res.status, StatusCode::Ok);

    let mut tmp = target.as_ref().to_str().unwrap().to_string();

    // Implement overwrite strategy
    if Path::new(&tmp).exists() {
      use OverwriteStrategy::*;

      println!("Target {} already exists. {}", tmp, match strategy {
        Skip => "Skipping",
        Overwrite => "Replacing",
      });

      match strategy {
        Skip => return Ok(()),
        Overwrite => (),
      }
    }

    tmp.push_str(".incomplete");

    {
      let mut out = try!(File::create(&tmp));
      try!(io::copy(&mut res, &mut out));
      try!(out.sync_all());
    }

    try!(fs::rename(tmp, target));
    Ok(())
  }
}

#[test]
fn test_from_row() {
  for row in vec!["/DCIM/100OLYMP,P4270171.ORF,14845727,0,18587,40165",
                  "/DCIM/100OLYMP,P4270171.JPG,7935748,0,18587,40165",
                  "/DCIM/100OLYMP,P4270172.ORF,14877614,0,18587,40167",
                  "/DCIM/100OLYMP,P4270172.JPG,8023494,0,18587,40167",
                  "/DCIM/100OLYMP,P4270173.ORF,14894106,0,18587,40217",
                  "/DCIM/100OLYMP,P4270173.JPG,8203245,0,18587,40217",
                  "/DCIM/100OLYMP,P4270174.ORF,14936402,0,18587,40225"] {
    println!("{:?}", TransferItem::from_row(row));
  }
}

fn request_list(client: &Client, endpoint: &str) -> Result<Vec<TransferItem>> {
  let mut url = BASE_URL.to_string();
  url.push_str(endpoint);

  debug!("fetching listing at {:?}", url);

  let mut res = try!(client.get(&url).send());
  assert_eq!(res.status, StatusCode::Ok);

  let mut body = String::new();
  try!(res.read_to_string(&mut body));
  let mut rows = body.split("\r\n");

  let version = rows.next().expect("Invalid camera response");

  if version != "VER_100" {
    return Err(Error::ProtocolError)
  }

  let rows = rows
    .filter(|row| !row.is_empty())
    .map(|row| TransferItem::from_row(&row))
    .collect();

  Ok(rows)
}

fn list_items(client: &Client) -> Result<Vec<TransferItem>> {
  fn list_rec(client: &Client, dir: &str) -> Result<LinkedList<TransferItem>> {
    let endpoint = format!("get_imglist.cgi?DIR={}", dir);
    let entries = try!(request_list(&client, &endpoint));

    let mut files = LinkedList::new();

    for entry in entries {
      if entry.is_directory() {
        files.append(&mut try!(list_rec(&client, &entry.path())));
      } else {
        files.push_back(entry);
      }
    }
    Ok(files)
  }

  let entries = try!(list_rec(&client, "/DCIM")).into_iter().collect();
  Ok(entries)
}

pub trait Transfer: Sized {
  fn from_config(c: &Config) -> Option<Self>;
  fn download_directory(&self) -> &PathBuf;

  fn items(&self, client: &Client) -> Result<Vec<TransferItem>>;
  fn item_downloaded(&self, _item: &TransferItem) -> Result<()> { Ok(()) }
}

// pub fn power_off() -> Result<()> {
//   let mut url = BASE_URL.to_string();
//   url.push_str("exec_pwoff.cgi");
//   debug!("GET {}", url);

//   try!(Client::new().get(&url).send());
//   Ok(())
// }

pub fn execute_transfer<T: Transfer>(transfer: T, config: &Config) -> Result<()> {
  let client = Client::new();

  let entries = try!(transfer.items(&client));
  let dir = transfer.download_directory().to_path_buf();
  try!(fs::create_dir_all(&dir));

  // Used for formatting
  let pad_width = format!("{}", entries.len()).len();

  for (i,entry) in entries.iter().enumerate() {
    let mut target = dir.clone();
    target.push(&entry.filename);
    println!("[{i:>pad$}/{len}] Downloading {filename} to {target}",
             pad      = pad_width,
             i        = i+1,
             len      = entries.len(),
             filename = entry.filename,
             target   = target.display());

    let result = entry.download(&client, &target, config.overwrite_strategy);
    if result.is_err() {
      warn!("Failed to download {}", entry.filename);
      if config.error_strategy == ErrorStrategy::Abort {
        return result;
      };
    };

    try!(transfer.item_downloaded(&entry))
  }

  Ok(())
}

pub struct OrderTransfer {
  download_dir: PathBuf
}

impl Transfer for OrderTransfer {
  fn from_config(c: &Config) -> Option<Self> {
    c.transfer_order_dir.as_ref().map(|d| OrderTransfer {
      download_dir: d.clone()
    })
  }

  fn download_directory(&self) -> &PathBuf {
    &self.download_dir
  }

  fn items(&self, client: &Client) -> Result<Vec<TransferItem>> {
    println!("Checking for transfer order items...");
    let entries = try!(request_list(&client, "get_rsvimglist.cgi"));
    println!("Got {} items in transfer order", entries.len());
    Ok(entries)
  }
}

pub struct IncrementalTransfer {
  download_dir: PathBuf,
  state_file: PathBuf,
}

const DATE_FORMAT: &'static str = "%Y-%m-%dT%H:%M:%S";

impl IncrementalTransfer {
  fn last_download_date(&self) -> Option<NaiveDateTime> {
    use std::io::ErrorKind;
    match File::open(&self.state_file) {
      Err(ref e) if e.kind() == ErrorKind::NotFound => None,
      Err(ref e) => panic!(e.to_string()),
      Ok(mut f) => {
        let mut buf = String::new();
        f.read_to_string(&mut buf).expect("Failed to read from state file");
        let date = NaiveDateTime::parse_from_str(&buf.trim(), DATE_FORMAT)
          .expect("Corrup history file");
        debug!("read date from state file: {}", date);
        Some(date)
      }
    }
  }

  fn store_download_date(&self, date: &NaiveDateTime) -> io::Result<()> {
    // TODO: Make sure we don't write an older date (can happen for camera-developed ORFs)
    let mut f = try!(File::create(&self.state_file));
    try!(f.write_fmt(format_args!("{}", date.format(DATE_FORMAT))));
    try!(f.sync_all());
    Ok(())
  }
}

impl Transfer for IncrementalTransfer {
  fn from_config(c: &Config) -> Option<Self> {
    c.download_dir.as_ref().map(|dir| {
      let mut state_file = dir.clone();
      state_file.push("omd-downloader.state");

      IncrementalTransfer {
        download_dir: dir.clone(),
        state_file: state_file,
      }
    })
  }

  fn download_directory(&self) -> &PathBuf {
    &self.download_dir
  }

  fn items(&self, client: &Client) -> Result<Vec<TransferItem>> {
    println!("Checking for new files...");

    let last_downloaded = self.last_download_date();
    let entries = try!(list_items(&client));

    let entries: Vec<_> = match last_downloaded {
      None => entries.into_iter().collect(),
      Some(date) => entries.into_iter()
        .filter(|e| e.date > date)
        .collect()
    };

    println!("Got {} new files to download", entries.len());
    Ok(entries)
  }

  fn item_downloaded(&self, item: &TransferItem) -> Result<()> {
    try!(self.store_download_date(&item.date));

    Ok(())
  }
}
