extern crate omd_transfer;
extern crate env_logger;
#[macro_use] extern crate log;
extern crate getopts;

use omd_transfer::*;

use getopts::Options;
use std::{env, fs, io, process};
use std::io::{Write,ErrorKind};
use std::path::PathBuf;

fn print_usage(program: &str, opts: Options) {
  let brief = format!("Usage: {} [options]", program);
  print!("{}", opts.usage(&brief));
}

fn write_usage() -> io::Result<()> {
  let file = fs::OpenOptions::new()
    .write(true)
    .create_new(true)
    .open("config.toml");

  match file {
    Ok(mut file) => {
      try!(file.write_all(Config::template().as_bytes()));
      Ok(())
    },
    Err(e) => {
      if e.kind() == ErrorKind::AlreadyExists {
        println!("config.toml already exists.");
        process::exit(1);
      } else {
        Err(e)
      }
    }
  }
}

fn main() {
  env_logger::init().unwrap();

  let args: Vec<String> = env::args().collect();
  let program = args[0].clone();

  let mut opts = Options::new();
  opts.optopt("c", "config", "Config file to use. Defaults to ./config.toml", "FILE");
  opts.optflag("t", "write-template", "Print config template to stdout");
  opts.optflag("h", "help", "print this help menu");
  let matches = match opts.parse(&args[1..]) {
    Ok(m) => { m }
    Err(f) => { panic!(f.to_string()) }
  };

  if matches.opt_present("h") {
    print_usage(&program, opts);
    return;
  }

  if matches.opt_present("t") {
    write_usage().expect("Failed to write config template");
    println!("Wrote config template to config.toml");
    return;
  }

  let config_file: PathBuf = matches.opt_str("c")
    .or(env::var("OMD_TRANSFER_CONFIG").ok())
    .unwrap_or("config.toml".into())
    .into();

  if !config_file.exists() {
    println!("File {} not found", config_file.display());
    return;
  }

  let config = Config::from_file(&config_file);

  let f = || {
    OrderTransfer::from_config(&config).map(|transfer| {
      info!("Starting to execute transfer order");
      execute_transfer(transfer, &config).unwrap();
    });

    IncrementalTransfer::from_config(&config).map(|transfer| {
      info!("Starting to execute incremental transfer");
      execute_transfer(transfer, &config).unwrap();
    });

    if config.power_off {
      println!("Powering off...");
      power_off().expect("Failed to power off the cmaera");
    }
  };

  // Workaround for https://github.com/rust-lang/rust/issues/15701
  run_transfers(f);
}

#[cfg(not(feature = "dbus"))]
fn run_transfers<F: FnOnce() -> ()>(f: F) {
  f()
}

#[cfg(feature = "dbus")]
fn run_transfers<F: FnOnce() -> ()>(f: F) {
  let interface_name = "wlp3s0";
  let network_name = "E-M10MKII-P-BHLA37440";

  use omd_transfer::wifi;
  wifi::with_temporary_network(interface_name, network_name, f)
}
