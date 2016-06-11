use dbus::*;
// use dbus::arg::*;
// use dbus::obj::*;

pub fn test_dbus() {
  let c = Connection::get_private(BusType::System).unwrap();

  let interface_name = "wlp3s0";
  let network_name = "E-M10MKII-P-BHLA37440";
  
  if let Some(interface) = get_interface(&c, interface_name) {
    println!("Found interface!");
      
    let current_network = current_network(&c, &interface);
    println!("{:?}", current_network);

    let camera_network = find_network(&c, &interface, network_name);
    println!("{:?}", camera_network);
  }
}

fn associate_network<'a>(conn: &'a Connection, network: &)

fn current_network<'a>(conn: &'a Connection, interface: &Props<'a>) -> Option<Path<'a>> {
  if let Ok(MessageItem::ObjectPath(network)) = interface.get("CurrentNetwork") {
    Some(network)
  } else {
    None
  }
}

fn find_network<'a>(conn: &'a Connection,
                    interface: &Props<'a>,
                    network_name: &str) -> Option<Path<'a>> {
  use dbus::MessageItem::*;

  let network_name = format!("\"{}\"", network_name);
  
  if let Ok(Array(networks, _)) = interface.get("Networks") {
    for network in networks {
      if let MessageItem::ObjectPath(network) = network {
        let p = Props::new(conn,
                           "fi.w1.wpa_supplicant1",
                           network.clone(),
                           "fi.w1.wpa_supplicant1.Network",
                           100);
        if let Ok(Array(props, _)) = p.get("Properties") {
          for prop in props {
            if let DictEntry(box Str(prop_name), box Variant(box Str(prop_val))) = prop {
              println!("{}: {:?}", prop_name, prop_val);
              if prop_name == "ssid" && prop_val == network_name {
                return Some(network)
              }
            }
          }
        }
      }
    }
  }

  None
}

fn get_interface<'a>(conn: &'a Connection, interface_name: &str) -> Option<Props<'a>> {
  let p = Props::new(conn,
                     "fi.w1.wpa_supplicant1",
                     "/fi/w1/wpa_supplicant1",
                     "fi.w1.wpa_supplicant1",
                     1000);


  if let Ok(MessageItem::Array(interfaces, _)) = p.get("Interfaces") {
    for path in interfaces {
      if let MessageItem::ObjectPath(path) = path {
        let ip = Props::new(conn,
                            "fi.w1.wpa_supplicant1",
                            path,
                            "fi.w1.wpa_supplicant1.Interface",
                            1000);
        if let Ok(MessageItem::Str(ifname)) = ip.get("Ifname") {
          if ifname == interface_name {
            return Some(ip)
          }
        }
      }
    }
  }

  None
}
