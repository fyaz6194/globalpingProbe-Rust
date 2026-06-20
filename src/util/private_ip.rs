use ipnet::IpNet;
use once_cell::sync::Lazy;
use std::net::IpAddr;
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
    if get_local_ips().contains(&ip) {
        return true;
    }
    PRIVATE_RANGES.iter().any(|net| net.contains(&ip))
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
}
