/// mDNS service advertisement for Stream Deck Studio.
///
/// The node-elgato-stream-deck discovery service (discoveryService.ts) listens for:
///   type: 'elg', protocol: 'tcp'   →  _elg._tcp.local.
///
/// TXT record fields used by convertService():
///   vid  : vendorId as decimal string  (e.g. "4057" for 0x0fd9)
///   pid  : productId as decimal string (e.g. "170" for 0x00aa)
///   sn   : serial number
///   dt   : device type (215 = Network Dock; anything else → use vid/pid)
use mdns_sd::{ServiceDaemon, ServiceInfo};
use tracing::{error, info};

use crate::studio::{DT_VALUE, PRODUCT_ID, VENDOR_ID};

pub struct MdnsAdvertiser {
    daemon: ServiceDaemon,
    fullname: String,
}

impl MdnsAdvertiser {
    /// Start advertising the service.
    pub fn start(
        instance_name: &str,
        port: u16,
        serial: &str,
    ) -> anyhow::Result<Self> {
        let daemon = ServiceDaemon::new()?;

        let service_type = "_elg._tcp.local.";
        let host_name = format!("{}.local.", gethostname());
        let properties = [
            ("vid", format!("{}", VENDOR_ID)),
            ("pid", format!("{}", PRODUCT_ID)),
            ("sn", serial.to_string()),
            ("dt", DT_VALUE.to_string()),
        ];

        let info = ServiceInfo::new(
            service_type,
            instance_name,
            &host_name,
            "",      // ip resolved by daemon
            port,
            &properties[..],
        )?;

        let fullname = info.get_fullname().to_string();
        daemon.register(info)?;

        info!(
            "mDNS: advertising '{}' on port {} (vid={} pid={} sn={})",
            fullname,
            port,
            VENDOR_ID,
            PRODUCT_ID,
            serial,
        );

        Ok(Self { daemon, fullname })
    }

    pub fn stop(&self) {
        if let Err(e) = self.daemon.unregister(&self.fullname) {
            error!("mDNS unregister failed: {:?}", e);
        }
    }
}

impl Drop for MdnsAdvertiser {
    fn drop(&mut self) {
        self.stop();
    }
}

fn gethostname() -> String {
    hostname::get()
        .ok()
        .and_then(|h| h.into_string().ok())
        .unwrap_or_else(|| "streamdeck-emulator".to_string())
}
