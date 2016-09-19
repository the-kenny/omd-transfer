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

fn write_config_template() -> io::Result<()> {
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
  opts.optopt("c", "config", "Config file to use. Defaults to ~/.herbstmove.toml", "FILE");
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
    write_config_template().expect("Failed to write config template");
    println!("Wrote config template to config.toml");
    return;
  }

  let config_file: PathBuf = matches.opt_str("c")
    .or(env::var("OMD_TRANSFER_CONFIG").ok())
    .map(PathBuf::from)
    .unwrap_or({
      let mut homedir = env::home_dir().expect("Couldn't get home dir");
      homedir.push(".omd-transfer.toml");
      homedir
    });
  let config_file = config_file.canonicalize()
    .expect("Couldn't canonicalize config_file");

  if !config_file.exists() {
    println!("File {} not found", config_file.display());
    return;
  }

  println!("Using config from {}", config_file.display());

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
  };

  // Workaround for https://github.com/rust-lang/rust/issues/15701
  run_transfers(&config, f);
}

#[cfg(not(feature = "dbus"))]
fn run_transfers<F: FnOnce() -> ()>(config: &Config, f: F) {
  if config.wifi.is_some() {
    panic!("Found `wifi` section in config but compiled without DBUS support");
  }

  f()
}

use std::panic;
#[cfg(feature = "dbus")]
fn run_transfers<F: FnOnce() -> () + panic::UnwindSafe>(config: &Config, f: F) {
  match config.wifi {
    Some(ref config) => {
      use omd_transfer::wifi;
      wifi::with_temporary_network(&config, f)
    },
    None => f()
  }
}
