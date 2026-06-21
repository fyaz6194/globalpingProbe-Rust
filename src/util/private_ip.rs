use ipnet::IpNet;
use once_cell::sync::Lazy;
use std::net::{IpAddr, Ipv4Addr};
use std::collections::HashSet;

// All RFC-reserved ranges — mirrors src/lib/private-ip.ts in the Node.js probe
static PRIVATE_RANGES: Lazy<Vec<IpNet>> = Lazy::new(|| {
    [
        // IPv4
        "0.0.0.0/8", "10.0.0.0/8", "100.64.0.0/10", "127.0.0.0/8",
        "169.254.0.0/16", "172.16.0.0/12", "192.0.0.0/24", "192.0.2.0/24",
        "192.88.99.0/24", "192.168.0.0/16", "198.18.0.0/15", "198.51.100.0/24",
        "203.0.113.0/24", "224.0.0.0/4", "240.0.0.0/4", "255.255.255.255/32",
        // IPv6
        "::/128", "::1/128", "64:ff9b:1::/48", "100::/64",
        "2001::/32", "2001:10::/28", "2001:20::/28", "2001:db8::/32",
        "2002::/16", "fc00::/7", "fe80::/10", "ff00::/8",
    ]
    .iter()
    .map(|s| s.parse().expect("invalid CIDR range"))
    .collect()
});

pub fn is_ip_private(ip: IpAddr) -> bool {
    // Normalise IPv6 forms that embed an IPv4 address so that e.g.
    // `::ffff:127.0.0.1` (IPv4-mapped) or `64:ff9b::a.b.c.d` (NAT64) cannot be
    // used to smuggle a private IPv4 target past the range filter. Without this,
    // `::ffff:169.254.169.254` would reach cloud metadata endpoints (SSRF).
    let ip = canonicalize(ip);

    if get_local_ips().contains(&ip) {
        return true;
    }
    PRIVATE_RANGES.iter().any(|net| net.contains(&ip))
}

/// Collapse IPv6 addresses that embed an IPv4 address down to that IPv4 address
/// so range checks apply to the address actually routed to.
///
/// Handles:
/// - IPv4-mapped IPv6 (`::ffff:a.b.c.d`)
/// - NAT64 well-known prefix (`64:ff9b::/96`)
///
/// All other addresses are returned unchanged.
fn canonicalize(ip: IpAddr) -> IpAddr {
    let IpAddr::V6(v6) = ip else { return ip };

    if let Some(v4) = v6.to_ipv4_mapped() {
        return IpAddr::V4(v4);
    }

    let seg = v6.segments();
    // 64:ff9b::/96 — NAT64 well-known prefix; embedded IPv4 is the low 32 bits.
    if seg[0] == 0x0064 && seg[1] == 0xff9b && seg[2..6] == [0, 0, 0, 0] {
        let o = v6.octets();
        return IpAddr::V4(Ipv4Addr::new(o[12], o[13], o[14], o[15]));
    }

    ip
}

fn get_local_ips() -> HashSet<IpAddr> {
    use std::net::UdpSocket;
    let mut ips = HashSet::new();
    // Probe the OS for the outbound IP — lightweight, no external crate needed yet
    if let Ok(sock) = UdpSocket::bind("0.0.0.0:0") {
        let _ = sock.connect("8.8.8.8:80");
        if let Ok(addr) = sock.local_addr() {
            ips.insert(addr.ip());
        }
    }
    ips
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn private_ipv4_ranges_are_blocked() {
        assert!(is_ip_private("10.0.0.1".parse().unwrap()));
        assert!(is_ip_private("192.168.1.1".parse().unwrap()));
        assert!(is_ip_private("172.16.0.1".parse().unwrap()));
        assert!(is_ip_private("127.0.0.1".parse().unwrap()));
    }

    #[test]
    fn public_ipv4_is_allowed() {
        assert!(!is_ip_private("1.1.1.1".parse().unwrap()));
        assert!(!is_ip_private("8.8.8.8".parse().unwrap()));
    }

    #[test]
    fn private_ipv6_ranges_are_blocked() {
        assert!(is_ip_private("::1".parse().unwrap()));
        assert!(is_ip_private("fc00::1".parse().unwrap()));
        assert!(is_ip_private("fe80::1".parse().unwrap()));
    }

    #[test]
    fn ipv4_mapped_ipv6_private_is_blocked() {
        // Regression: these must not bypass the filter via the IPv6 encoding.
        assert!(is_ip_private("::ffff:127.0.0.1".parse().unwrap()));
        assert!(is_ip_private("::ffff:10.0.0.1".parse().unwrap()));
        assert!(is_ip_private("::ffff:192.168.1.1".parse().unwrap()));
        assert!(is_ip_private("::ffff:169.254.169.254".parse().unwrap())); // cloud metadata
    }

    #[test]
    fn ipv4_mapped_ipv6_public_is_allowed() {
        // Mapped *public* addresses must still be permitted (normalise, don't blanket-block).
        assert!(!is_ip_private("::ffff:1.1.1.1".parse().unwrap()));
        assert!(!is_ip_private("::ffff:8.8.8.8".parse().unwrap()));
    }

    #[test]
    fn nat64_embedded_private_is_blocked() {
        // 64:ff9b::169.254.169.254 → 169.254.169.254
        assert!(is_ip_private("64:ff9b::a9fe:a9fe".parse().unwrap()));
        // 64:ff9b::10.0.0.1
        assert!(is_ip_private("64:ff9b::a00:1".parse().unwrap()));
    }
}
