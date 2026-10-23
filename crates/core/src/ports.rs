use crate::{ErrorKind, ManagementError, Port};
use std::net::IpAddr;

/// Compare published sockets; wildcard binds overlap specific interfaces.
pub fn ports_overlap(a: &Port, b: &Port) -> bool {
    if a.host_port != b.host_port || a.protocol != b.protocol {
        return false;
    }
    let (Ok(a), Ok(b)) = (a.host.parse::<IpAddr>(), b.host.parse::<IpAddr>()) else {
        return false;
    };
    a == b
        || (a.is_unspecified() && (a.is_ipv6() || b.is_ipv4()))
        || (b.is_unspecified() && (b.is_ipv6() || a.is_ipv4()))
}

pub fn validate_ports(ports: &[Port]) -> Result<(), ManagementError> {
    if ports.len() > 32 {
        return Err(ManagementError::new(
            ErrorKind::InvalidData,
            "最多配置 32 条端口映射",
        ));
    }
    for (index, port) in ports.iter().enumerate() {
        if port.host.parse::<IpAddr>().is_err()
            || port.host_port == 0
            || port.container_port == 0
            || !["tcp", "udp"].contains(&port.protocol.as_str())
        {
            return Err(ManagementError::new(
                ErrorKind::InvalidData,
                format!(
                    "第 {} 条映射无效：填写有效 IP、1–65535 端口和 TCP/UDP 协议",
                    index + 1
                ),
            ));
        }
        if ports[..index].iter().any(|p| ports_overlap(p, port)) {
            return Err(ManagementError::new(
                ErrorKind::Conflict,
                format!("第 {} 条映射的主机端口重复或与通配地址冲突", index + 1),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn port(host: &str, protocol: &str) -> Port {
        Port {
            host: host.into(),
            host_port: 3000,
            container_port: 80,
            protocol: protocol.into(),
        }
    }
    #[test]
    fn published_socket_validation_respects_protocol_and_bind_address() {
        assert!(validate_ports(&[port("127.0.0.1", "tcp"), port("127.0.0.1", "udp")]).is_ok());
        assert!(validate_ports(&[port("127.0.0.1", "tcp"), port("127.0.0.2", "tcp")]).is_ok());
        assert!(validate_ports(&[port("0.0.0.0", "tcp"), port("127.0.0.1", "tcp")]).is_err());
        assert!(validate_ports(&[port("::", "tcp"), port("::1", "tcp")]).is_err());
        assert!(ports_overlap(
            &port("::1", "tcp"),
            &port("0:0:0:0:0:0:0:1", "tcp")
        ));
        assert!(!ports_overlap(&port("0.0.0.0", "tcp"), &port("::1", "tcp")));
        for host in ["localhost", "127.999.0.1", "::::", "[::1]"] {
            assert!(validate_ports(&[port(host, "tcp")]).is_err(), "{host}");
        }
        let mut zero = port("127.0.0.1", "tcp");
        zero.container_port = 0;
        assert!(validate_ports(&[zero]).is_err());
        assert!(validate_ports(&[port("127.0.0.1", "sctp")]).is_err());
        assert!(validate_ports(&[]).is_ok());
    }
}
