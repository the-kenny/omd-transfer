use std::{fs, io};
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
  let day   =  date      & 0b00011111;
  let month =  date >> 5 & 0b00001111;
  let year  = (date >> 9 & 0b01111111) + 1980;

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

  pub fn download<P: AsRef<Path>>(&self, client: &Client, target: &P) -> Result<()> {
    let url = format!("{}{}", BASE_URL, self.path());
    debug!("Fetching {}", url);
    let mut res = try!(client.get(&url).send());
    assert_eq!(res.status, StatusCode::Ok);

    let mut tmp = target.as_ref().to_str().unwrap().to_string();
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
  fn list_rec(client: &Client, dir: &str, mut acc: &mut Vec<TransferItem>) -> Result<()> {
    let endpoint = format!("get_imglist.cgi?DIR={}", dir);
    let entries = try!(request_list(&client, &endpoint));
    acc.reserve(entries.len());
    for entry in entries {
      if entry.is_directory() {
        try!(list_rec(&client, &entry.path(), &mut acc));
      } else {
        acc.push(entry);
      }
    }
    Ok(())
  }

  let mut entries = vec![];
  try!(list_rec(&client, "/DCIM", &mut entries));
  Ok(entries)
}

fn list_transfer_order(client: &Client) -> Result<Vec<TransferItem>> {
  request_list(&client, "get_rsvimglist.cgi")
}

pub struct Transfer {
  download_dir: PathBuf,
  state_file: PathBuf,
  error_strategy: ErrorStrategy,
  http_client: Client,
}

const DATE_FORMAT: &'static str = "%Y-%m-%dT%H:%M:%S";

impl Transfer {
  pub fn from_config(config: &Config) -> Option<Self> {
    config.download_dir.clone().map(|download_dir| {
      // TODO: Allow users to specify separate path via config
      let mut state_file = download_dir.clone();
      state_file.push("omd-downloader.state");

      Transfer {
        download_dir: download_dir,
        state_file: state_file,
        error_strategy: config.error_strategy,
        http_client: Client::new(),
      }
    })
  }

  pub fn download_new(&self) -> Result<()> {
    let last_downloaded = self.last_download_date();
    let entries = try!(list_items(&self.http_client));
    let entries: Vec<_> = match last_downloaded {
      None => entries.iter().collect(),
      Some(date) => entries.iter()
        .filter(|e| e.date > date)
        .collect()
    };

    info!("Got {} files to download", entries.len());

    for entry in entries {
      let mut target = self.download_dir.clone();
      target.push(&entry.filename);
      info!("Downloading {} to {:?}", entry.filename, target);
      
      let result = entry.download(&self.http_client, &target);
      if result.is_err() {
        warn!("Failed to download {}", entry.filename);
        if self.error_strategy == ErrorStrategy::Abort {
          return result;
        };
      };
      
      try!(self.store_download_date(&entry.date));
    }

    Ok(())
  }

  // pub fn download_transfer_order(&self) -> Result<()> {
  //   let download_dir = self.config.transfer_order_dir.clone()
  //     .expect("No transfer_order_directory configured");
  
  //   debug!("Checking for transfer order...");
  //   let entries = try!(request_list(&self.http_client, "get_rsvimglist.cgi"));
  //   info!("Got {} items in transfer order", entries.len());

  //   for entry in entries {
  //     let mut target = download_dir
  //       .clone();
  //     target.push(&entry.filename);
  //     info!("Downloading {} to {:?}", entry.filename, target);
  //     try!(entry.download(&self.http_client, &target));
  //   }

  //   Ok(())
  // }

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
        info!("read date from state file: {}", date);
        Some(date)
      }
    }
  }

  fn store_download_date(&self, date: &NaiveDateTime) -> io::Result<()> {
    let mut f = try!(File::create(&self.state_file));
    try!(f.write_fmt(format_args!("{}", date.format(DATE_FORMAT))));
    try!(f.sync_all());
    Ok(())
  }
}
