extern crate hyper;
extern crate chrono;
extern crate regex;
#[macro_use] extern crate lazy_static;

use chrono::*;

use std::fs;
use std::io;
use std::io::Read;
use std::fs::File;
use std::path::{Path,PathBuf};

use hyper::Client;
use hyper::header::Connection;
use regex::Regex;

const BASE_URL: &'static str = "http://192.168.0.10";

#[derive(Debug, PartialEq, Eq, Hash)]
struct TransferItem {
  pub parent: String,
  pub filename: String,
  pub file_size: u64,
  pub date: chrono::NaiveDateTime,
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

  pub fn download<P: AsRef<Path>>(&self, target: &P) -> std::io::Result<()> {
    let client = Client::new();

    let mut res = client.get(&format!("{}/{}", BASE_URL, self.path()))
      .header(Connection::close())
      .send().unwrap();

    let mut tmp = target.as_ref().to_path_buf();
    tmp.set_file_name("incomplete_download");

    {
      let mut out = try!(File::create(&tmp));
      try!(std::io::copy(&mut res, &mut out));
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

fn list_directory(dir: &str) -> hyper::Result<Vec<TransferItem>> {
  let client = Client::new();

  let mut res = try!(client.get(&format!("{}/get_imglist.cgi?DIR={}", BASE_URL, dir)).send());

  let mut body = String::new();
  res.read_to_string(&mut body).unwrap();

  let mut rows = body.split("\r\n");
  let version = rows.next().unwrap();
  assert_eq!(version, "VER_100");
  let rows = rows
    .filter(|row| !row.is_empty())
    .map(|row| TransferItem::from_row(&row))
    .collect();

  Ok(rows)
}

struct Transfer {
  download_dir: PathBuf,
  state_file: PathBuf,
  entries: Vec<TransferItem>,
}

impl Transfer {
  pub fn new<P: AsRef<Path>>(download_dir: P) -> Self {
    assert!(download_dir.as_ref().is_dir());

    let mut state_file = download_dir.as_ref().to_path_buf();
    state_file.push("omd-downloader.state");

    Transfer {
      download_dir: download_dir.as_ref().to_path_buf(),
      state_file: state_file,
      entries: Vec::new(),
    }
  }

  fn list_items(&mut self) -> hyper::Result<()> {
    println!("Fetching picture list from camera...");
    let mut acc = vec![];
    try!(self.list_rec("/DCIM", &mut acc));
    acc.sort_by_key(|e| e.date);
    println!("Got {} pictures", acc.len());
    self.entries = acc;
    Ok(())
  }

  fn last_download_date(&self) -> Option<NaiveDateTime> {
    use std::io::ErrorKind;
    match File::open(&self.state_file) {
      Err(ref e) if e.kind() == ErrorKind::NotFound => None,
      Err(ref e) => panic!(e.to_string()),
      Ok(mut f) => {
        use std::str::FromStr;

        let mut buf = String::new();
        f.read_to_string(&mut buf).expect("Failed to read from state file");
        let ts: i64 = i64::from_str(&buf).expect("Corrupt state file");
        let date = NaiveDateTime::from_timestamp(ts, 0);
        println!("read date from state file: {}", date);
        Some(date)
      }
    }
  }

  fn store_download_date(&self, date: &NaiveDateTime)
                                         -> io::Result<()> {
    use std::io::Write;

    let mut f = try!(File::create(&self.state_file));
    try!(f.write_fmt(format_args!("{}", date.timestamp())));
    try!(f.sync_all());
    Ok(())
  }

  pub fn download_new(&self) -> io::Result<()> {
    let last_downloaded = self.last_download_date();
    let entries: Vec<_> = match last_downloaded {
      None => self.entries.iter().collect(),
      Some(date) => self.entries.iter()
        .filter(|e| e.date > date)
        .collect()
    };

    for entry in entries {
      let mut target = self.download_dir.clone();
      target.push(&entry.filename);
      println!("Downloading {} to {:?}", entry.filename, target);
      try!(entry.download(&target));
      try!(self.store_download_date(&entry.date));
    }

    Ok(())
  }

  fn list_rec(&mut self, dir: &str, mut acc: &mut Vec<TransferItem>) -> hyper::Result<()> {
    for entry in try!(list_directory(dir)) {
      if entry.is_directory() {
        try!(self.list_rec(&entry.path(), &mut acc));
      } else {
        acc.push(entry);
      }
    }
    Ok(())
  }
}

fn main() {
  let mut transfer = Transfer::new("foo/");
  transfer.list_items().unwrap();
  transfer.download_new().unwrap();
}
