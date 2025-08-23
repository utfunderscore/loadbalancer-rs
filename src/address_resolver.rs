use std::net::IpAddr;
use std::str::FromStr;

use hickory_resolver::{
    TokioAsyncResolver,
    config::{ResolverConfig, ResolverOpts},
    error::ResolveError,
    proto::rr::rdata::SRV,
};
use rand::Rng;
use rand::seq::SliceRandom;

#[derive(Debug, thiserror::Error)]
pub enum EndpointError {
    #[error("DNS resolve error: {0}")]
    Resolve(#[from] ResolveError),
    #[error("No A/AAAA records found for {0}")]
    NoAddress(String),
    #[error("Invalid host:port format")]
    InvalidHostPort,
    #[error("SRV lookup failed and no fallback available")]
    NoSrvAndNoFallback,
}

#[derive(Debug, Clone)]
pub struct ResolvedEndpoint {
    pub ip: String,
    pub port: u16,
    pub original_input: String,
    pub resolved_host: String,
}

pub async fn resolve_host_port(
    input: &str,
    service: &str,
    proto: &str,
    fallback_port: u16,
) -> Result<ResolvedEndpoint, EndpointError> {
    let resolver = TokioAsyncResolver::tokio(ResolverConfig::default(), ResolverOpts::default());

    if let Some((host_part, port)) = split_host_port(input)? {
        println!("host: {}", host_part);

        if let Ok(ip) = IpAddr::from_str(host_part) {
            return Ok(ResolvedEndpoint {
                ip: ip.to_string(),
                port,
                original_input: input.to_string(),
                resolved_host: host_part.to_string(),
            });
        }

        let addrs = resolver.lookup_ip(host_part).await?;
        if let Some(ip) = addrs.iter().next() {
            return Ok(ResolvedEndpoint {
                ip: ip.to_string(),
                port,
                original_input: input.to_string(),
                resolved_host: host_part.to_string(),
            });
        } else {
            return Err(EndpointError::NoAddress(host_part.to_string()));
        }
    }

    let host = normalize_host_without_port(input);

    if let Ok(ip) = IpAddr::from_str(&host) {
        return Ok(ResolvedEndpoint {
            ip: ip.to_string(),
            port: fallback_port,
            original_input: input.to_string(),
            resolved_host: host,
        });
    }

    let has_alpha = host.chars().any(|c| c.is_ascii_alphabetic());
    if has_alpha {
        let srv_name = format!(
            "_{}._{}.{}",
            service.trim_start_matches('_'),
            proto.trim_start_matches('_'),
            host
        );

        if let Ok(answers) = resolver.srv_lookup(&srv_name).await {
            let srv_records: Vec<&SRV> = answers.iter().collect();
            if let Some(chosen) = pick_srv(&srv_records) {
                let target = chosen.target().to_utf8().trim_end_matches('.').to_string();
                let addrs = target.parse().map_err(|_| EndpointError::InvalidHostPort)?;
                return Ok(ResolvedEndpoint {
                    ip: addrs,
                    port: chosen.port(),
                    original_input: input.to_string(),
                    resolved_host: target,
                });
            }
        }

        let addrs = resolver.lookup_ip(&host).await?;
        if let Some(ip) = addrs.iter().next() {
            return Ok(ResolvedEndpoint {
                ip: ip.to_string(),
                port: fallback_port,
                original_input: input.to_string(),
                resolved_host: host,
            });
        } else {
            return Err(EndpointError::NoAddress(host));
        }
    }

    Err(EndpointError::NoSrvAndNoFallback)
}

// RFC 2782 selection (priority + weight)
fn pick_srv<'a>(records: &'a [&'a SRV]) -> Option<&'a SRV> {
    if records.is_empty() {
        return None;
    }
    let min_priority = records.iter().map(|r| r.priority()).min()?;
    let mut same_prio: Vec<&SRV> = records
        .iter()
        .copied()
        .filter(|r| r.priority() == min_priority)
        .collect();

    let total_weight: u32 = same_prio.iter().map(|r| r.weight() as u32).sum();
    if total_weight == 0 {
        // Uniform shuffle
        let mut rng = rand::thread_rng();
        same_prio.shuffle(&mut rng);
        return same_prio.into_iter().next();
    }

    let mut rng = rand::thread_rng();
    let mut pick = rng.gen_range(0..total_weight);
    for r in same_prio {
        let w = r.weight() as u32;
        if pick < w {
            return Some(r);
        }
        pick -= w;
    }
    None
}

fn split_host_port(input: &str) -> Result<Option<(&str, u16)>, EndpointError> {
    if input.starts_with('[') {
        return if let Some(end) = input.find(']') {
            let host = &input[1..end];
            if end + 1 < input.len() {
                if &input[end + 1..end + 2] != ":" {
                    return Err(EndpointError::InvalidHostPort);
                }
                let port_str = &input[end + 2..];
                let port: u16 = port_str
                    .parse()
                    .map_err(|_| EndpointError::InvalidHostPort)?;
                Ok(Some((host, port)))
            } else {
                Ok(None)
            }
        } else {
            Err(EndpointError::InvalidHostPort)
        };
    }

    let colon_count = input.matches(':').count();
    if colon_count == 0 {
        return Ok(None);
    }
    if colon_count > 1 && IpAddr::from_str(input).is_ok() {
        return Ok(None); // IPv6 literal without brackets
    }

    if let Some(idx) = input.rfind(':') {
        let host = &input[..idx];
        let port_str = &input[idx + 1..];
        if host.is_empty() || port_str.is_empty() {
            return Err(EndpointError::InvalidHostPort);
        }
        let port: u16 = port_str
            .parse()
            .map_err(|_| EndpointError::InvalidHostPort)?;
        Ok(Some((host, port)))
    } else {
        Ok(None)
    }
}

fn normalize_host_without_port(input: &str) -> String {
    let h = input.trim();
    h.strip_suffix('.').unwrap_or(h).to_string()
}
