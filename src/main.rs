extern crate omd_transfer;
extern crate env_logger;
#[macro_use] extern crate log;
extern crate getopts;

use omd_transfer::*;

use getopts::Options;
use std::env;
use std::path::PathBuf;

fn print_usage(program: &str, opts: Options) {
  let brief = format!("Usage: {} [options]", program);
  print!("{}", opts.usage(&brief));
}

fn main() {
  env_logger::init().unwrap();

  let args: Vec<String> = env::args().collect();
  let program = args[0].clone();

  let mut opts = Options::new();
  opts.optopt("c", "config", "Config file to use", "FILE");
  opts.optflag("h", "help", "print this help menu");
  let matches = match opts.parse(&args[1..]) {
    Ok(m) => { m }
    Err(f) => { panic!(f.to_string()) }
  };

  if matches.opt_present("h") {
    print_usage(&program, opts);
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
}
