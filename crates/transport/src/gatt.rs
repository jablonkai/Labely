//! Pure GATT helpers shared by the btleplug transport and the native mobile shells:
//! known NIIMBOT UUIDs (docs/protocol.md §3.1), the split serial service of other vendors
//! (docs/protocol-languages.md §7.2), scan filtering and characteristic choice.

/// Standard NIIMBOT service.
pub const NIIMBOT_SERVICE: &str = "e7810a71-73ae-499d-8c15-faa9aef0c3f2";
/// Standard NIIMBOT characteristic (notify + write-without-response, both directions).
pub const NIIMBOT_CHARACTERISTIC: &str = "bef8d6c9-9c21-4c9e-b632-bd58c1009f9f";

/// Known (service, characteristic) pairs, in preference order. The second pair names the
/// characteristic only by its 16-bit alias `FFE1`; see [`uuid_matches`].
pub const KNOWN_PAIRS: [(&str, &str); 3] = [
    (NIIMBOT_SERVICE, NIIMBOT_CHARACTERISTIC),
    ("0000e0ff-3c17-d293-8e48-14fe2e4da212", "ffe1"),
    (
        "49535343-fe7d-4ae5-8fa9-9fafd205e455",
        "49535343-8841-43f4-a8d4-ecbe34729bb3",
    ),
];

/// Recommended pause between packets on BLE (protocol.md §3.1).
pub const BLE_PACKET_GAP_MS: u64 = 10;

/// Write payload size guaranteed by the default ATT MTU (23 − 3 bytes header).
pub const BLE_DEFAULT_PAYLOAD: usize = 20;

/// One GATT characteristic as seen after service discovery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CharCandidate {
    pub service: String,
    pub characteristic: String,
    pub notify: bool,
    pub write_without_response: bool,
}

impl CharCandidate {
    fn usable(&self) -> bool {
        self.notify && self.write_without_response
    }
}

/// Compare a full UUID against a pattern that is either a full UUID or a 16-bit alias
/// (`"ffe1"`), which matches `0000ffe1-…` under any base UUID. Case-insensitive.
pub fn uuid_matches(uuid: &str, pattern: &str) -> bool {
    if pattern.len() == 4 {
        let u = uuid.to_ascii_lowercase();
        u.len() >= 8 && u.starts_with("0000") && u[4..8].eq_ignore_ascii_case(pattern)
    } else {
        uuid.eq_ignore_ascii_case(pattern)
    }
}

/// Pick the characteristic to use for both directions. Order: a known pair (in
/// [`KNOWN_PAIRS`] order), then any notify+write-without-response characteristic in a known
/// service, then any notify+write-without-response characteristic at all. Returns an index
/// into `chars`.
pub fn select_characteristic(chars: &[CharCandidate]) -> Option<usize> {
    for (svc, ch) in KNOWN_PAIRS {
        if let Some(i) = chars.iter().position(|c| {
            c.usable() && uuid_matches(&c.service, svc) && uuid_matches(&c.characteristic, ch)
        }) {
            return Some(i);
        }
    }
    for (svc, _) in KNOWN_PAIRS {
        if let Some(i) = chars
            .iter()
            .position(|c| c.usable() && uuid_matches(&c.service, svc))
        {
            return Some(i);
        }
    }
    chars.iter().position(CharCandidate::usable)
}

/// Serial service of Phomemo and Marklife printers (16-bit alias): host writes go to
/// [`SPLIT_WRITE`], replies arrive on the other characteristics of the service.
pub const SPLIT_SERVICE: &str = "ff00";
/// Write characteristic of [`SPLIT_SERVICE`].
pub const SPLIT_WRITE: &str = "ff02";

/// The characteristics of a connection: one to write to, one or more to listen on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    /// Index into the candidates.
    pub write: usize,
    /// Indexes into the candidates. Everything they notify is one byte stream.
    pub notify: Vec<usize>,
}

/// Pick the characteristics of a connection: one characteristic for both directions if
/// [`select_characteristic`] finds it, otherwise the split serial service.
pub fn select_link(chars: &[CharCandidate]) -> Option<Link> {
    if let Some(i) = select_characteristic(chars) {
        return Some(Link {
            write: i,
            notify: vec![i],
        });
    }
    let in_service = |c: &CharCandidate| uuid_matches(&c.service, SPLIT_SERVICE);
    let write = chars.iter().position(|c| {
        in_service(c) && c.write_without_response && uuid_matches(&c.characteristic, SPLIT_WRITE)
    })?;
    let notify: Vec<usize> = (0..chars.len())
        .filter(|&i| in_service(&chars[i]) && chars[i].notify)
        .collect();
    (!notify.is_empty()).then_some(Link { write, notify })
}

/// Scan filter: keep a device that advertises the NIIMBOT service, or whose name starts
/// with one of `name_prefixes` (case-insensitive). Empty `name_prefixes` disables the name
/// filter, so every device is kept.
pub fn scan_matches(name: Option<&str>, services: &[String], name_prefixes: &[&str]) -> bool {
    if name_prefixes.is_empty() || services.iter().any(|s| uuid_matches(s, NIIMBOT_SERVICE)) {
        return true;
    }
    let Some(name) = name else { return false };
    let name = name.to_ascii_lowercase();
    name_prefixes
        .iter()
        .any(|p| name.starts_with(&p.to_ascii_lowercase()))
}

/// Usable write payload for an ATT MTU (MTU − 3), never below [`BLE_DEFAULT_PAYLOAD`].
pub fn payload_for_att_mtu(att_mtu: u16) -> usize {
    usize::from(att_mtu)
        .saturating_sub(3)
        .max(BLE_DEFAULT_PAYLOAD)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(service: &str, characteristic: &str, notify: bool, wwr: bool) -> CharCandidate {
        CharCandidate {
            service: service.into(),
            characteristic: characteristic.into(),
            notify,
            write_without_response: wwr,
        }
    }

    #[test]
    fn prefers_standard_pair() {
        let chars = [
            c(
                "49535343-FE7D-4AE5-8FA9-9FAFD205E455",
                "49535343-8841-43F4-A8D4-ECBE34729BB3",
                true,
                true,
            ),
            c(
                "0000180a-0000-1000-8000-00805f9b34fb",
                "00002a29-0000-1000-8000-00805f9b34fb",
                true,
                true,
            ),
            c(
                "E7810A71-73AE-499D-8C15-FAA9AEF0C3F2",
                "BEF8D6C9-9C21-4C9E-B632-BD58C1009F9F",
                true,
                true,
            ),
        ];
        assert_eq!(select_characteristic(&chars), Some(2));
    }

    #[test]
    fn alias_pair_and_property_requirements() {
        let chars = [
            c(
                "0000e0ff-3c17-d293-8e48-14fe2e4da212",
                "0000ffe1-0000-1000-8000-00805f9b34fb",
                true,
                false,
            ),
            c(
                "0000e0ff-3c17-d293-8e48-14fe2e4da212",
                "0000ffe1-3c17-d293-8e48-14fe2e4da212",
                true,
                true,
            ),
        ];
        assert_eq!(select_characteristic(&chars), Some(1));
    }

    #[test]
    fn falls_back_to_known_service_then_any() {
        let in_known = [
            c("aaaa0000-0000-0000-0000-000000000000", "bbbb", true, true),
            c(
                NIIMBOT_SERVICE,
                "12345678-0000-0000-0000-000000000000",
                true,
                true,
            ),
        ];
        assert_eq!(select_characteristic(&in_known), Some(1));
        let any = [
            c("aaaa0000-0000-0000-0000-000000000000", "x", false, true),
            c("aaaa0000-0000-0000-0000-000000000000", "y", true, true),
        ];
        assert_eq!(select_characteristic(&any), Some(1));
        assert_eq!(select_characteristic(&any[..1]), None);
        assert_eq!(select_characteristic(&[]), None);
    }

    #[test]
    fn split_service_link() {
        let svc = "0000ff00-0000-1000-8000-00805f9b34fb";
        let ch = |n: &str, notify, wwr| {
            c(
                svc,
                &format!("0000{n}-0000-1000-8000-00805f9b34fb"),
                notify,
                wwr,
            )
        };
        // Marklife: ff01 and ff03 notify, ff02 takes writes.
        let chars = [
            c("0000180a-0000-1000-8000-00805f9b34fb", "2a29", true, false),
            ch("ff01", true, false),
            ch("ff02", false, true),
            ch("ff03", true, false),
        ];
        assert_eq!(select_characteristic(&chars), None);
        assert_eq!(
            select_link(&chars),
            Some(Link {
                write: 2,
                notify: vec![1, 3]
            })
        );
        // No write characteristic, or nothing to listen on.
        assert_eq!(select_link(&chars[..2]), None);
        assert_eq!(select_link(&chars[2..3]), None);
        // One characteristic for both directions still wins.
        let both = [
            ch("ff02", false, true),
            c(NIIMBOT_SERVICE, NIIMBOT_CHARACTERISTIC, true, true),
        ];
        assert_eq!(
            select_link(&both),
            Some(Link {
                write: 1,
                notify: vec![1]
            })
        );
    }

    #[test]
    fn uuid_alias_matching() {
        assert!(uuid_matches("0000FFE1-0000-1000-8000-00805f9b34fb", "ffe1"));
        assert!(!uuid_matches(
            "1234ffe1-0000-1000-8000-00805f9b34fb",
            "ffe1"
        ));
        assert!(!uuid_matches(
            "ffe1",
            "0000ffe1-0000-1000-8000-00805f9b34fb"
        ));
    }

    #[test]
    fn scan_filter() {
        let none: Vec<String> = vec![];
        let niim = vec![NIIMBOT_SERVICE.to_ascii_uppercase()];
        assert!(scan_matches(None, &none, &[]));
        assert!(scan_matches(None, &niim, &["B21"]));
        assert!(scan_matches(Some("b21_pro-1234"), &none, &["B21", "D11"]));
        assert!(!scan_matches(Some("Headphones"), &none, &["B21"]));
        assert!(!scan_matches(None, &none, &["B21"]));
    }

    #[test]
    fn payload_sizes() {
        assert_eq!(payload_for_att_mtu(23), 20);
        assert_eq!(payload_for_att_mtu(0), 20);
        assert_eq!(payload_for_att_mtu(185), 182);
    }
}
