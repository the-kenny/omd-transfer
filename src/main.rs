extern crate hyper;
extern crate chrono;
extern crate regex;
#[macro_use] extern crate lazy_static;

use chrono::*;

use std::io;
use std::io::Read;
use std::fs::File;
use std::path::{Path,PathBuf};

use hyper::Client;
use hyper::header::Connection;
use regex::Regex;

use std::collections::HashSet;

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
    let mut f = try!(File::create(target));
    let client = Client::new();

    let mut res = client.get(&format!("{}/{}", BASE_URL, self.path()))
      .header(Connection::close())
      .send().unwrap();

    try!(std::io::copy(&mut res, &mut f));
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

fn list_directory(dir: &str) -> Vec<TransferItem> {
  let client = Client::new();

  let mut res = client.get(&format!("{}/get_imglist.cgi?DIR={}", BASE_URL, dir))
    .header(Connection::close())
    .send().unwrap();

  let mut body = String::new();
  res.read_to_string(&mut body).unwrap();

  let mut rows = body.split("\r\n");
  let version = rows.next().unwrap();
  assert_eq!(version, "VER_100");
  rows
    .filter(|row| !row.is_empty())
    .map(|row| TransferItem::from_row(&row))
    .collect()
}

struct Transfer {
  camera_dir: String,
  entries: HashSet<TransferItem>,
}

impl Transfer {
  pub fn new(camera_dir: &str) -> Self {
    Transfer {
      camera_dir: camera_dir.to_string(),
      entries: HashSet::new(),
    }
  }

  pub fn list_all(&mut self) {
    let dir = self.camera_dir.clone();
    self.list_rec(&dir);
  }

  fn state_file<P: AsRef<Path>>(dir: P) -> PathBuf {
    let mut state_file: PathBuf = dir.as_ref().to_path_buf();
    state_file.push("omd-downloader.state");
    state_file
  }
  
  fn last_download_date<P: AsRef<Path>>(target_dir: P) -> Option<NaiveDateTime> {
    use std::io::ErrorKind;
    match File::open(&Self::state_file(target_dir)) {
      Err(ref e) if e.kind() == ErrorKind::NotFound => None,
      Err(ref e) => panic!(e.to_string()),
      Ok(mut f) => {
        let mut buf = String::new();
        f.read_to_string(&mut buf).expect("Failed to read from state file");
        use std::str::FromStr;
        let date = NaiveDateTime::from_str(&buf).expect("Corrypt state file");
        println!("read date from state file: {}", date);
        Some(date)
      }
    }
  }

  fn store_download_date<P: AsRef<Path>>(target_dir: P, date: &NaiveDateTime)
                                         -> io::Result<()> {
    use std::io::Write;
    
    let mut f = try!(File::create(&Self::state_file(target_dir)));
    try!(f.write_fmt(format_args!("{}", date)));
    try!(f.sync_all());
    Ok(())
  }

  pub fn download_all<P: AsRef<Path>>(&self, target_dir: P) -> io::Result<()> {
    let last_downloaded = Self::last_download_date(&target_dir);
    let mut entries: Vec<_> = match last_downloaded {
      None => self.entries.iter().collect(),
      Some(date) => self.entries.iter()
        .filter(|e| e.date > date)
        .collect()
    };

    entries.sort_by_key(|e| e.date);

    for entry in entries {
      let mut target = target_dir.as_ref().to_path_buf();
      target.push(&entry.filename);
      println!("Downloading {} to {:?}", entry.filename, target);
      try!(entry.download(&target));
      try!(Self::store_download_date(&target_dir, &entry.date));
    }

    Ok(())
  }

  fn list_rec(&mut self, dir: &str) {
    for entry in list_directory(dir) {
      if entry.is_directory() {
        self.list_rec(&entry.path());
      } else {
        self.entries.insert(entry);
      }
    }
  }
}

fn main() {
  let mut transfer = Transfer::new("/DCIM");
  transfer.list_all();

  transfer.download_all("foo/");
}
