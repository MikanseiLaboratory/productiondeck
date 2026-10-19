//! mDNS discovery and advertisement for `_elg._tcp.local`.
//!
//! The official app finds Cora devices through mDNS. TXT keys observed on a
//! real Network Dock capture:
//! `dt, mf, md, pv, id, sn, fw, hwrev` (dock) and
//! `dt=213, vid=0x0FD9, pid=0x0080, prt=2` (child deck).
//! Studio values must be confirmed by capture (U2).

use std::collections::HashSet;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};

pub const SERVICE_TYPE: &str = "_elg._tcp.local.";

/// mDNS advertisement configuration.
#[derive(Debug, Clone)]
pub struct DiscoveryConfig {
    pub instance: String,
    pub port: u16,
    pub serial: String,
    pub model_name: String,
    pub firmware: String,
    /// Device-type TXT value. `215` is the Network Dock; Studio uses the
    /// captured value (provisional `213`).
    pub dt: String,
    pub vid: String,
    pub pid: String,
    pub hwrev: String,
    /// Child-port capability / protocol revision byte (`prt`), if advertised.
    pub prt: Option<String>,
    /// MAC address used for the `id` TXT key (dock capture uses `id=<MAC>`).
    pub mac: Option<String>,
}

impl Default for DiscoveryConfig {
    fn default() -> Self {
        Self {
            instance: "Stream Deck Studio".to_string(),
            port: 5343,
            serial: "PRODUCTIONDK".to_string(),
            model_name: "Stream Deck Studio".to_string(),
            firmware: "1.00.000".to_string(),
            dt: "213".to_string(),
            vid: "0x0FD9".to_string(),
            pid: "0x00AA".to_string(),
            hwrev: "0200".to_string(),
            prt: Some("2".to_string()),
            mac: None,
        }
    }
}

impl DiscoveryConfig {
    fn txt(&self) -> Vec<(&'static str, String)> {
        let mut props = vec![
            ("dt", self.dt.clone()),
            ("mf", "Elgato".to_string()),
            ("md", self.model_name.clone()),
            ("pv", "1.0".to_string()),
            ("sn", self.serial.clone()),
            ("vid", self.vid.clone()),
            ("pid", self.pid.clone()),
            ("fw", self.firmware.clone()),
            ("hwrev", self.hwrev.clone()),
        ];
        if let Some(prt) = &self.prt {
            props.push(("prt", prt.clone()));
        }
        if let Some(mac) = &self.mac {
            props.push(("id", mac.clone()));
        }
        props
    }
}

/// A running mDNS advertisement.
pub struct Advertisement {
    daemon: ServiceDaemon,
    fullname: String,
}

impl Advertisement {
    /// Start advertising `_elg._tcp`.
    pub fn start(config: &DiscoveryConfig) -> Result<Self> {
        let daemon = ServiceDaemon::new().context("create mDNS daemon")?;
        let host_name = format!(
            "{}.local.",
            std::env::var("COMPUTERNAME")
                .or_else(|_| std::env::var("HOSTNAME"))
                .unwrap_or_else(|_| "productiondeck".to_string())
                .replace(' ', "-")
        );
        let ip = local_ipv4().map(|ip| ip.to_string()).unwrap_or_default();
        let props = config.txt();
        let info = ServiceInfo::new(
            SERVICE_TYPE,
            &config.instance,
            &host_name,
            ip.as_str(),
            config.port,
            &props[..],
        )
        .context("build mDNS service info")?;
        let fullname = info.get_fullname().to_string();
        daemon.register(info).context("register mDNS service")?;
        Ok(Self { daemon, fullname })
    }

    pub fn fullname(&self) -> &str {
        &self.fullname
    }
}

impl Drop for Advertisement {
    fn drop(&mut self) {
        let _ = self.daemon.unregister(&self.fullname);
        let _ = self.daemon.shutdown();
    }
}

/// A discovered `_elg._tcp` service.
#[derive(Debug, Clone)]
pub struct Discovered {
    pub fullname: String,
    pub host: String,
    pub port: u16,
    pub addresses: Vec<IpAddr>,
    pub txt: Vec<(String, String)>,
}

impl Discovered {
    pub fn txt_value(&self, key: &str) -> Option<&str> {
        self.txt
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    pub fn is_network_dock(&self) -> bool {
        self.txt_value("dt") == Some("215")
    }
}

/// Browse for `_elg._tcp` services for `timeout`.
pub fn discover(timeout: Duration) -> Result<Vec<Discovered>> {
    let daemon = ServiceDaemon::new().context("create mDNS daemon")?;
    let receiver = daemon.browse(SERVICE_TYPE).context("browse _elg._tcp")?;
    let deadline = Instant::now() + timeout;
    let mut seen = HashSet::new();
    let mut out = Vec::new();

    while Instant::now() < deadline {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        match receiver.recv_timeout(remaining) {
            Ok(ServiceEvent::ServiceResolved(info)) => {
                let fullname = info.get_fullname().to_string();
                if !seen.insert(fullname.clone()) {
                    continue;
                }
                let txt = info
                    .get_properties()
                    .iter()
                    .map(|p| (p.key().to_string(), p.val_str().to_string()))
                    .collect();
                let addresses = info
                    .get_addresses_v4()
                    .into_iter()
                    .map(|ip| IpAddr::V4(*ip))
                    .collect();
                out.push(Discovered {
                    fullname,
                    host: info.get_hostname().to_string(),
                    port: info.get_port(),
                    addresses,
                    txt,
                });
            }
            Ok(_) => {}
            Err(_) => break,
        }
    }

    let _ = daemon.shutdown();
    Ok(out)
}

fn local_ipv4() -> Option<Ipv4Addr> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    match socket.local_addr().ok()? {
        SocketAddr::V4(addr) => Some(*addr.ip()),
        SocketAddr::V6(_) => None,
    }
}
