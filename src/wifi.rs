use dbus;
use dbus::{Connection,Path,Props,Message,BusType,MessageItem};
use std::{result,thread};
use std::time::Duration;
use std::cell::RefCell;

#[derive(Debug)]
enum Error {
  Timeout,
  Dbus(dbus::Error),
}

impl From<dbus::Error> for Error {
  fn from(other: dbus::Error) -> Self {
    Error::Dbus(other)
  }
}

type Result<T> = result::Result<T,Error>;

struct WifiInterface<'a> {
  conn: &'a Connection,
  path: Path<'a>,
  props: Props<'a>,
  name: String,
}

impl<'a> WifiInterface<'a> {
  fn find(conn: &'a Connection, interface_name: &str) -> Option<Self> {
    let p = Props::new(&conn,
                       "fi.w1.wpa_supplicant1",
                       "/fi/w1/wpa_supplicant1",
                       "fi.w1.wpa_supplicant1",
                       1000);

    if let Ok(MessageItem::Array(interfaces, _)) = p.get("Interfaces") {
      for path in interfaces {
        if let MessageItem::ObjectPath(path) = path {
          let ip = Props::new(&conn,
                              "fi.w1.wpa_supplicant1",
                              path.clone(),
                              "fi.w1.wpa_supplicant1.Interface",
                              1000);

          if let Ok(MessageItem::Str(ifname)) = ip.get("Ifname") {
            if ifname == interface_name {
              return Some(WifiInterface {
                conn: conn,
                path: path,
                props: ip,
                name: interface_name.to_string(),
              })
            }
          }
        }
      }
    }

    None
  }

  fn current_network(&'a self) -> Option<WifiNetwork<'a>> {
    if let Ok(MessageItem::ObjectPath(network)) = self.props.get("CurrentNetwork") {
      Some(WifiNetwork::new(network, self))
    } else {
      None
    }
  }

  fn state(&'a self) -> Result<String> {
    let props = Props::new(&self.conn,
                           "fi.w1.wpa_supplicant1",
                           self.path.clone(),
                           "fi.w1.wpa_supplicant1.Interface",
                           1000);

    let state: MessageItem = try!(props.get("State"));
    let state: &str = state.inner().unwrap();
    Ok(state.to_string())
  }

  fn find_network(&'a self, name: &str) -> Option<WifiNetwork<'a>> {
    use dbus::MessageItem::*;

    let props = Props::new(&self.conn,
                           "fi.w1.wpa_supplicant1",
                           self.path.clone(),
                           "fi.w1.wpa_supplicant1.Interface",
                           1000);

    if let Ok(Array(networks, _)) = props.get("Networks") {
      for network in networks {
        if let MessageItem::ObjectPath(network) = network {
          let network = WifiNetwork::new(network, &self);
          if network.ssid() == name {
            return Some(network)
          }
        }
      }
    }

    None
  }

  pub fn is_up(&self) -> bool {
    use get_if_addrs;
    get_if_addrs::get_if_addrs().unwrap().iter()
      .find(|i| i.name == self.name)
      .is_some()
  }
}

struct WifiNetwork<'a> {
  path: Path<'a>,
  interface: &'a WifiInterface<'a>,
  ssid: RefCell<Option<String>>,
}

impl<'a> WifiNetwork<'a> {
  fn new(network: Path<'a>,
         interface: &'a WifiInterface<'a>) -> Self {
    WifiNetwork {
      path: network,
      interface: interface,
      ssid: RefCell::new(None),
    }
  }

  fn ssid(&self) -> String {
    use dbus::MessageItem::*;

    match *self.ssid.borrow() {
      Some(ref ssid) => return ssid.clone(),
      _ => (),
    }
    
    let p = Props::new(self.interface.conn,
                       "fi.w1.wpa_supplicant1",
                       self.path.clone(),
                       "fi.w1.wpa_supplicant1.Network",
                       100);

    if let Ok(Array(props, _)) = p.get("Properties") {
      for prop in props {
        let (prop, val) = prop.inner().unwrap();
        let prop_name: &str = prop.inner().unwrap();

        if prop_name == "ssid" {
          let val: &MessageItem = val.inner().unwrap();
          let val: &str = val.inner().unwrap();
          let mut val = val.to_string();
          if val.starts_with("\"") { val.remove(0); }
          if val.ends_with("\"") {
            let len = val.len();
            val.remove(len-1);
          }
          *self.ssid.borrow_mut() = Some(val.clone());
          return val;
        }
      }
    }
    unreachable!()
  }

  fn associate(&self, timeout: Duration) -> Result<()> {
    println!("Associating with {}", self.ssid());
    
    let msg = Message::new_method_call("fi.w1.wpa_supplicant1",
                                       self.interface.path.clone(),
                                       "fi.w1.wpa_supplicant1.Interface",
                                       "SelectNetwork")
      .unwrap()
      .append1(self.path.clone());
    try!(self.interface.conn.send_with_reply_and_block(msg, 1000));

    let sleep = Duration::from_millis(200);
    let mut spent = Duration::from_millis(0);
    while try!(self.interface.state()) != "completed" {
      thread::sleep(sleep);

      spent += sleep;
      if spent > timeout {
        return Err(Error::Timeout)
      }
    }

    println!("Associated!");
    return Ok(())
  }
}

pub fn with_temporary_network<F>(interface_name: &str, network_name: &str, f: F) -> ()
  where F: FnOnce() -> () {
  let c = Connection::get_private(BusType::System).unwrap();

  let interface = WifiInterface::find(&c, interface_name).unwrap();
  let original_network = interface.current_network().unwrap();
  println!("Original network: {}", original_network.ssid());
  
  let camera_network = interface.find_network(&network_name).unwrap();

  let timeout = Duration::from_secs(30);
  camera_network.associate(timeout).unwrap();
  println!("Waiting for camera to become available ({}s timeout)...",
           timeout.as_secs());
  while !interface.is_up() {
    thread::sleep(Duration::from_millis(500));
  }

  // TODO: Use 1.10's new unwind-API
  f();

  println!("Reconnecting to old network...");
  original_network.associate(timeout).unwrap();
}
