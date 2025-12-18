
use std::{sync::Arc, time::Duration, collections::{HashMap, HashSet}};

use axum::{extract::State, routing::get, Json, Router};
use serde::Serialize;
use tokio::{net::TcpListener, sync::RwLock, time::sleep};
use std::net::{IpAddr, SocketAddr};
use std::fs::File;
use std::io::{BufRead, BufReader};

#[derive(Debug, Clone, Serialize, Hash, PartialEq, Eq)]
pub struct Connection {
    pub proto: String,
    pub src_ip: IpAddr,
    pub src_port: u16,
    pub dst_ip: IpAddr,
    pub dst_port: u16,
    pub state: String,
    pub bytes: u64,
    pub throughput_bytes_per_sec: u64,
}

type SharedConnections = Arc<RwLock<Vec<Connection>>>;

#[derive(Clone)]
struct AppState {
    connections: SharedConnections,
    node_name: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let conntrack_env = std::env::var("CONNTRACK_PATH").unwrap_or_else(|_| "auto".into());
    let conntrack_path = resolve_conntrack_path(&conntrack_env);

    if std::env::var("KFLOW_DEBUG").is_ok() {
        eprintln!("kflow daemon starting; CONNTRACK_PATH={} -> {}", conntrack_env, if conntrack_path.is_empty() { "(no candidate found yet)".to_string() } else { conntrack_path.clone() });
    }

    let state: SharedConnections = Arc::new(RwLock::new(Vec::new()));
    let node_name = std::env::var("KUBE_NODE_NAME").ok().or_else(|| {
        hostname::get().ok().and_then(|h| h.into_string().ok())
    });
    
    let state_clone = state.clone();
    let path = conntrack_path.clone();
    tokio::spawn(async move {
        let mut prev_set: HashSet<Connection> = HashSet::new();
        let mut prev_bytes: HashMap<(String, String, u16, String, u16), u64> = HashMap::new();
        let sample_interval = 2u64;
        
        loop {
            let mut flows = read_conntrack(&path);

            for flow in &mut flows {
                let key = (
                    flow.proto.clone(),
                    flow.src_ip.to_string(),
                    flow.src_port,
                    flow.dst_ip.to_string(),
                    flow.dst_port,
                );
                
                if let Some(&prev_byte_count) = prev_bytes.get(&key) {
                    let byte_delta = flow.bytes.saturating_sub(prev_byte_count);
                    flow.throughput_bytes_per_sec = byte_delta / sample_interval;
                }
                
                prev_bytes.insert(key, flow.bytes);
            }
            
            let new_set: HashSet<Connection> = flows.iter().cloned().collect();
            for added in new_set.difference(&prev_set) {
                println!("Added connection: {:?}", added);
            }
            for removed in prev_set.difference(&new_set) {
                println!("Removed connection: {:?}", removed);
            }
            prev_set = new_set;

            {
                let mut w = state_clone.write().await;
                *w = flows;
            }
            sleep(Duration::from_secs(sample_interval)).await;
        }
    });

    let app_state = AppState { connections: state, node_name };
    let app = Router::new()
        .route("/connections", get(list_connections))
        .with_state(app_state);

    let addr: SocketAddr = "0.0.0.0:8080".parse().unwrap();
    println!("Listening on {addr}");
    let listener = TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

fn resolve_conntrack_path(requested: &str) -> String {
    use std::path::Path;

    if requested == "auto" {
        if let Some(found) = detect_conntrack_candidate() {
            if std::env::var("KFLOW_DEBUG").is_ok() {
                eprintln!("auto-detected conntrack path: {}", found);
            }
            return found;
        }
        return String::new();
    }

    if Path::new(requested).exists() {
        return requested.to_string();
    }

    let alt_candidates = [
        format!("/host{}", requested),
        "/host/proc/net/nf_conntrack".into(),
        "/host/proc/net/ip_conntrack".into(),
        "/proc/net/nf_conntrack".into(),
        "/proc/net/ip_conntrack".into(),
    ];
    for c in &alt_candidates {
        if Path::new(c).exists() {
            if std::env::var("KFLOW_DEBUG").is_ok() {
                eprintln!("resolved conntrack path '{}' -> '{}'", requested, c);
            }
            return c.to_string();
        }
    }

    eprintln!("conntrack path '{}' does not exist and no alternatives found; will retry detection periodically", requested);
    String::new()
}

fn detect_conntrack_candidate() -> Option<String> {
    use std::path::Path;
    use std::fs::File;
    use std::io::BufRead;

    let candidates = [
        "/host/proc/net/nf_conntrack",
        "/host/proc/net/ip_conntrack",
        "/host/proc/net/nf_conntrack6",
        "/proc/net/nf_conntrack",
        "/proc/net/ip_conntrack",
        "/proc/net/nf_conntrack6",
        "/proc/net/ip_conntrack6",
    ];

    for c in &candidates {
        let p = Path::new(c);
        if !p.exists() {
            continue;
        }

        match File::open(p) {
            Ok(f) => {
                let mut reader = std::io::BufReader::new(f);
                let mut line = String::new();
                match reader.read_line(&mut line) {
                    Ok(n) => {
                        if n > 0 && !line.trim().is_empty() {
                            if std::env::var("KFLOW_DEBUG").is_ok() {
                                eprintln!("candidate {} exists and has content", c);
                            }
                            return Some(c.to_string());
                        } else {
                            if std::env::var("KFLOW_DEBUG").is_ok() {
                                eprintln!("candidate {} exists but was empty; skipping", c);
                            }
                            continue;
                        }
                    }
                    Err(e) => {
                        if std::env::var("KFLOW_DEBUG").is_ok() {
                            eprintln!("failed reading {}: {}", c, e);
                        }
                        continue;
                    }
                }
            }
            Err(e) => {
                if std::env::var("KFLOW_DEBUG").is_ok() {
                    eprintln!("failed opening {}: {}", c, e);
                }
                continue;
            }
        }
    }

    None
}

#[derive(Debug, Serialize)]
struct ConnectionsResponse {
    node_name: Option<String>,
    connections: Vec<Connection>,
}

async fn list_connections(
    State(state): State<AppState>,
) -> Json<ConnectionsResponse> {
    let snapshot = state.connections.read().await;
    Json(ConnectionsResponse {
        node_name: state.node_name.clone(),
        connections: snapshot.clone(),
    })
}

fn read_conntrack(path: &str) -> Vec<Connection> {
    read_conntrack_file(path)
}

fn read_conntrack_file(path: &str) -> Vec<Connection> {
    let mut path_to_use = path.to_string();
    if path_to_use.is_empty() {
        if let Some(detected) = detect_conntrack_candidate() {
            path_to_use = detected;
            if std::env::var("KFLOW_DEBUG").is_ok() {
                eprintln!("detected conntrack path during read: {}", path_to_use);
            }
        } else {
            return vec![];
        }
    }

    let file = match File::open(&path_to_use) {
        Ok(f) => f,
        Err(e) => {
            if std::env::var("KFLOW_DEBUG").is_ok() {
                eprintln!("Failed to open conntrack file {}: {}", path_to_use, e);
            }
            return vec![];
        }
    };
    let reader = BufReader::new(file);

    let lines: Vec<String> = reader.lines().filter_map(|l| l.ok()).collect();

    if std::env::var("KFLOW_DEBUG").is_ok() {
        eprintln!("read_conntrack_file: read {} lines from {}", lines.len(), path_to_use);
    }

    lines
        .into_iter()
        .filter_map(|l| parse_conntrack_line(&l))
        .collect()
}

fn parse_conntrack_line(line: &str) -> Option<Connection> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 5 {
        return None;
    }

    let debug = std::env::var("KFLOW_DEBUG").is_ok();
    
    static SAMPLE_PRINTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if debug && !SAMPLE_PRINTED.swap(true, std::sync::atomic::Ordering::Relaxed) {
        eprintln!("Sample conntrack line: {}", line);
    }
    
    let mut proto: Option<String> = None;
    let mut state: Option<String> = None;
    let mut src_ip: Option<IpAddr> = None;
    let mut dst_ip: Option<IpAddr> = None;
    let mut src_port: Option<u16> = None;
    let mut dst_port: Option<u16> = None;
    let mut bytes = 0u64;

    for p in &parts {
        if proto.is_none() && (*p == "tcp" || *p == "udp") {
            proto = Some((*p).to_string());
        } else if p.starts_with("src=") && src_ip.is_none() {
            src_ip = p["src=".len()..].parse().ok();
        } else if p.starts_with("dst=") && dst_ip.is_none() {
            dst_ip = p["dst=".len()..].parse().ok();
        } else if p.starts_with("sport=") && src_port.is_none() {
            src_port = p["sport=".len()..].parse().ok();
        } else if p.starts_with("dport=") && dst_port.is_none() {
            dst_port = p["dport=".len()..].parse().ok();
        } else if p.starts_with("bytes=") {
            if let Ok(b) = p["bytes=".len()..].parse::<u64>() {
                bytes += b;
                if debug && bytes > 0 {
                    eprintln!("Found bytes={} in conntrack line", b);
                }
            }
        } else if state.is_none()
            && (*p == "ESTABLISHED"
                || *p == "SYN_SENT"
                || *p == "SYN_RECV"
                || *p == "FIN_WAIT"
                || *p == "TIME_WAIT")
        {
            state = Some((*p).to_string());
        }
    }

    Some(Connection {
        proto: proto?,
        src_ip: src_ip?,
        src_port: src_port?,
        dst_ip: dst_ip?,
        dst_port: dst_port?,
        state: state.unwrap_or_else(|| "UNKNOWN".into()),
        bytes,
        throughput_bytes_per_sec: 0,
    })
}