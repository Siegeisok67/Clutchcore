use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::util;

pub struct ProbeEndpoint {
    pub label: &'static str,
    pub provider: &'static str,
    pub region: &'static str,
    pub host: &'static str,
    pub port: u16,
}

pub static ENDPOINTS: &[ProbeEndpoint] = &[
    ProbeEndpoint {
        label: "Riot · Global edge",
        provider: "Riot",
        region: "Global anycast edge",
        host: "auth.riotgames.com",
        port: 443,
    },
    ProbeEndpoint {
        label: "Battle.net · US",
        provider: "Battle.net",
        region: "US",
        host: "us.actual.battle.net",
        port: 1119,
    },
    ProbeEndpoint {
        label: "Battle.net · EU",
        provider: "Battle.net",
        region: "EU",
        host: "eu.actual.battle.net",
        port: 1119,
    },
    ProbeEndpoint {
        label: "Battle.net · KR",
        provider: "Battle.net",
        region: "KR",
        host: "kr.actual.battle.net",
        port: 1119,
    },
    ProbeEndpoint {
        label: "Steam · Global",
        provider: "Steam",
        region: "Global API",
        host: "api.steampowered.com",
        port: 443,
    },
    ProbeEndpoint {
        label: "Epic · Global",
        provider: "Epic",
        region: "Global launcher",
        host: "launcher.epicgames.com",
        port: 443,
    },
    ProbeEndpoint {
        label: "EA · Global",
        provider: "EA",
        region: "Global API",
        host: "accounts.ea.com",
        port: 443,
    },
    ProbeEndpoint {
        label: "Ubisoft · Global",
        provider: "Ubisoft",
        region: "Global API",
        host: "public-ubiservices.ubi.com",
        port: 443,
    },
    ProbeEndpoint {
        label: "Call of Duty · Global",
        provider: "Call of Duty",
        region: "Global API",
        host: "profile.callofduty.com",
        port: 443,
    },
    ProbeEndpoint {
        label: "Minecraft · Global",
        provider: "Minecraft",
        region: "Global API",
        host: "api.minecraftservices.com",
        port: 443,
    },
];

pub fn endpoints() -> &'static [ProbeEndpoint] {
    ENDPOINTS
}

pub fn providers() -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    for endpoint in ENDPOINTS {
        if !out.contains(&endpoint.provider) {
            out.push(endpoint.provider);
        }
    }
    out
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ProbeResult {
    pub label: String,
    pub provider: String,
    pub region: String,
    pub host: String,
    pub port: u16,
    pub tested_unix: u64,
    pub ip: Option<String>,
    pub samples: Vec<f64>,
    pub dns_ms: f64,
    pub avg_ms: f64,
    pub min_ms: f64,
    pub max_ms: f64,
    pub jitter_ms: f64,
    pub loss_pct: f64,
    pub sent: usize,
    pub error: Option<String>,
}

impl ProbeResult {
    pub fn ok(&self) -> bool {
        self.error.is_none() && !self.samples.is_empty()
    }

    pub fn route_label(&self) -> &str {
        &self.label
    }
}

pub fn probe(
    label: &str,
    provider: &str,
    region: &str,
    host: &str,
    port: u16,
    samples: usize,
    timeout_ms: u64,
) -> ProbeResult {
    let samples = samples.clamp(1, 30);
    let mut result = ProbeResult {
        label: label.to_string(),
        provider: provider.to_string(),
        region: region.to_string(),
        host: host.to_string(),
        port,
        tested_unix: util::unix_now(),
        sent: samples,
        ..Default::default()
    };

    let resolve_start = Instant::now();
    let addrs: Vec<SocketAddr> = match (host, port).to_socket_addrs() {
        Ok(iter) => iter.collect(),
        Err(e) => {
            result.error = Some(format!("DNS lookup failed: {e}"));
            return result;
        }
    };
    result.dns_ms = resolve_start.elapsed().as_secs_f64() * 1000.0;
    let addr = match addrs.iter().find(|a| a.is_ipv4()).or_else(|| addrs.first()) {
        Some(addr) => *addr,
        None => {
            result.error = Some("DNS returned no usable address.".into());
            return result;
        }
    };
    result.ip = Some(addr.ip().to_string());

    let mut measured: Vec<f64> = Vec::with_capacity(samples);
    let timeout = Duration::from_millis(timeout_ms.clamp(150, 5000));
    for i in 0..samples {
        let start = Instant::now();
        match TcpStream::connect_timeout(&addr, timeout) {
            Ok(stream) => {
                measured.push(start.elapsed().as_secs_f64() * 1000.0);
                drop(stream);
            }
            Err(_) => {}
        }
        if i + 1 < samples {
            std::thread::sleep(Duration::from_millis(60));
        }
    }

    result.loss_pct = ((samples - measured.len()) as f64 / samples as f64) * 100.0;
    if measured.is_empty() {
        result.error = Some(format!(
            "No TCP handshake completed within {timeout_ms} ms per try. The endpoint may block probes or your route is down."
        ));
        return result;
    }

    let sum: f64 = measured.iter().sum();
    result.avg_ms = sum / measured.len() as f64;
    result.min_ms = measured.iter().cloned().fold(f64::INFINITY, f64::min);
    result.max_ms = measured.iter().cloned().fold(0.0, f64::max);
    let variance = measured
        .iter()
        .map(|v| {
            let d = v - result.avg_ms;
            d * d
        })
        .sum::<f64>()
        / measured.len() as f64;
    result.jitter_ms = variance.sqrt();
    result.samples = measured;
    result
}

pub fn custom_probe(input: &str, samples: usize) -> ProbeResult {
    match parse_target(input) {
        Ok((host, port)) => probe(&host, "Custom", "Custom host", &host, port, samples, 1200),
        Err(error) => ProbeResult {
            label: input.to_string(),
            provider: "Custom".into(),
            region: "Custom host".into(),
            host: input.to_string(),
            port: 0,
            tested_unix: util::unix_now(),
            sent: samples,
            error: Some(error),
            ..Default::default()
        },
    }
}

pub fn parse_target(input: &str) -> Result<(String, u16), String> {
    let mut text = input.trim();
    if text.is_empty() {
        return Err("Enter a hostname, for example 1.1.1.1 or game.example.com:443.".into());
    }
    for prefix in ["https://", "http://", "tcp://"] {
        if let Some(stripped) = text.strip_prefix(prefix) {
            text = stripped;
            break;
        }
    }
    if let Some((host, _)) = text.split_once('/') {
        text = host;
    }
    if text.is_empty() {
        return Err("That address is empty.".into());
    }
    if text.contains(char::is_whitespace) {
        return Err("Addresses cannot contain spaces.".into());
    }
    if let Some((host, port)) = text.rsplit_once(':') {
        if !host.contains(':') {
            if let Ok(port) = port.parse::<u16>() {
                if !host.trim().is_empty() {
                    return Ok((host.trim().to_string(), port));
                }
            }
        }
    }
    Ok((text.to_string(), 443))
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ProbeSummary {
    pub best_label: String,
    pub best_provider: String,
    pub best_ms: f64,
    pub best_host: String,
    pub best_jitter: f64,
    pub tested: usize,
    pub avg_jitter: f64,
    pub worst_loss: f64,
}

pub fn summarize(results: &[ProbeResult]) -> Option<ProbeSummary> {
    let good: Vec<&ProbeResult> = results.iter().filter(|r| r.ok()).collect();
    if good.is_empty() {
        return None;
    }
    let best = good
        .iter()
        .min_by(|a, b| a.avg_ms.partial_cmp(&b.avg_ms).unwrap_or(std::cmp::Ordering::Equal))?;
    let avg_jitter =
        good.iter().map(|r| r.jitter_ms).sum::<f64>() / good.len().max(1) as f64;
    let worst_loss = good.iter().map(|r| r.loss_pct).fold(0.0, f64::max);
    Some(ProbeSummary {
        best_label: best.label.clone(),
        best_provider: best.provider.clone(),
        best_ms: best.avg_ms,
        best_host: best.host.clone(),
        best_jitter: best.jitter_ms,
        tested: good.len(),
        avg_jitter,
        worst_loss,
    })
}

pub fn cache_path() -> PathBuf {
    util::data_dir().join("last-probe.json")
}

#[derive(Serialize, Deserialize, Default)]
struct ProbeCache {
    #[serde(default)]
    tested_unix: u64,
    #[serde(default)]
    results: Vec<ProbeResult>,
}

pub fn save_results(results: &[ProbeResult]) {
    let cache = ProbeCache {
        tested_unix: util::unix_now(),
        results: results.to_vec(),
    };
    if let Ok(json) = serde_json::to_string_pretty(&cache) {
        let _ = util::write_text(&cache_path(), &json);
    }
}

pub fn load_results() -> Vec<ProbeResult> {
    std::fs::read_to_string(cache_path())
        .ok()
        .and_then(|text| serde_json::from_str::<ProbeCache>(&text).ok())
        .map(|cache| {
            cache
                .results
                .into_iter()
                .filter(|r| r.tested_unix > 0)
                .collect()
        })
        .unwrap_or_default()
}

