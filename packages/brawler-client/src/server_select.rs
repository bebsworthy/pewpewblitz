//! Direct-connect address parsing and bounded resolver primitives.

use std::net::{IpAddr, SocketAddr};

pub const DEFAULT_SERVER_PORT: u16 = 5000;
pub const MAX_LOGICAL_ADDRESS_BYTES: usize = 255;
pub const MAX_RESOLVED_CANDIDATES: usize = 4;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ServerAddressHost {
    Ip(IpAddr),
    Dns(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct LogicalServerAddress {
    pub host: ServerAddressHost,
    pub port: u16,
    canonical: String,
}

impl LogicalServerAddress {
    #[must_use]
    pub fn canonical(&self) -> &str {
        &self.canonical
    }

    #[must_use]
    pub fn numeric_socket(&self) -> Option<SocketAddr> {
        match self.host {
            ServerAddressHost::Ip(ip) => Some(SocketAddr::new(ip, self.port)),
            ServerAddressHost::Dns(_) => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AddressParseError {
    Empty,
    TooLong,
    InvalidCharacter,
    InvalidPort,
    InvalidIpv6Brackets,
    InvalidHost,
}

pub fn parse_server_address(input: &str) -> Result<LogicalServerAddress, AddressParseError> {
    let input = input.trim_matches(|character: char| character.is_ascii_whitespace());
    if input.is_empty() {
        return Err(AddressParseError::Empty);
    }
    if input.len() > MAX_LOGICAL_ADDRESS_BYTES {
        return Err(AddressParseError::TooLong);
    }
    if !input.is_ascii()
        || input
            .chars()
            .any(|character| character.is_control() || character.is_ascii_whitespace())
    {
        return Err(AddressParseError::InvalidCharacter);
    }

    if let Some(rest) = input.strip_prefix('[') {
        let close = rest
            .find(']')
            .ok_or(AddressParseError::InvalidIpv6Brackets)?;
        let address = &rest[..close];
        let suffix = &rest[close + 1..];
        let ip = address
            .parse::<std::net::Ipv6Addr>()
            .map(IpAddr::V6)
            .map_err(|_| AddressParseError::InvalidHost)?;
        let port = if suffix.is_empty() {
            DEFAULT_SERVER_PORT
        } else {
            parse_port(
                suffix
                    .strip_prefix(':')
                    .ok_or(AddressParseError::InvalidIpv6Brackets)?,
            )?
        };
        return Ok(logical_address(ServerAddressHost::Ip(ip), port));
    }
    if input.contains('[') || input.contains(']') {
        return Err(AddressParseError::InvalidIpv6Brackets);
    }

    if let Ok(ip) = input.parse::<IpAddr>() {
        return Ok(logical_address(
            ServerAddressHost::Ip(ip),
            DEFAULT_SERVER_PORT,
        ));
    }
    let colon_count = input.bytes().filter(|byte| *byte == b':').count();
    if colon_count > 1 {
        return Err(AddressParseError::InvalidIpv6Brackets);
    }
    let (host, port) = if colon_count == 1 {
        let (host, port) = input.rsplit_once(':').expect("one colon has two sides");
        (host, parse_port(port)?)
    } else {
        (input, DEFAULT_SERVER_PORT)
    };
    let host = if let Ok(ip) = host.parse::<IpAddr>() {
        ServerAddressHost::Ip(ip)
    } else {
        ServerAddressHost::Dns(validate_dns_name(host)?)
    };
    Ok(logical_address(host, port))
}

fn parse_port(value: &str) -> Result<u16, AddressParseError> {
    let port = value
        .parse::<u16>()
        .map_err(|_| AddressParseError::InvalidPort)?;
    if port == 0 {
        return Err(AddressParseError::InvalidPort);
    }
    Ok(port)
}

fn validate_dns_name(value: &str) -> Result<String, AddressParseError> {
    if value.is_empty() || value.len() > 253 || value.ends_with('.') {
        return Err(AddressParseError::InvalidHost);
    }
    for label in value.split('.') {
        if label.is_empty()
            || label.len() > 63
            || label.starts_with('-')
            || label.ends_with('-')
            || label
                .bytes()
                .any(|byte| !byte.is_ascii_alphanumeric() && byte != b'-')
        {
            return Err(AddressParseError::InvalidHost);
        }
    }
    Ok(value.to_ascii_lowercase())
}

fn logical_address(host: ServerAddressHost, port: u16) -> LogicalServerAddress {
    let canonical = match &host {
        ServerAddressHost::Ip(IpAddr::V4(ip)) => format!("{ip}:{port}"),
        ServerAddressHost::Ip(IpAddr::V6(ip)) => format!("[{ip}]:{port}"),
        ServerAddressHost::Dns(host) => format!("{host}:{port}"),
    };
    LogicalServerAddress {
        host,
        port,
        canonical,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn address_matrix_accepts_and_canonicalizes_supported_forms() {
        for (source, expected) in [
            ("127.0.0.1", "127.0.0.1:5000"),
            ("127.0.0.1:6000", "127.0.0.1:6000"),
            ("::1", "[::1]:5000"),
            ("[::1]", "[::1]:5000"),
            ("[::1]:6000", "[::1]:6000"),
            ("LOCALHOST", "localhost:5000"),
            ("play.Example.com:6000", "play.example.com:6000"),
        ] {
            assert_eq!(parse_server_address(source).unwrap().canonical(), expected);
        }
    }

    #[test]
    fn address_matrix_rejects_ambiguous_and_invalid_forms() {
        for source in [
            "",
            "host name",
            "localhost:0",
            "localhost:",
            "-host",
            "host-",
            "host..name",
            "[127.0.0.1]",
            "[::1",
            "::ffff:gggg",
            "localhost.",
        ] {
            assert!(parse_server_address(source).is_err(), "accepted {source:?}");
        }
        assert!(parse_server_address(&"a".repeat(256)).is_err());
        assert!(parse_server_address(&format!("{}.com", "a".repeat(64))).is_err());
    }
}
