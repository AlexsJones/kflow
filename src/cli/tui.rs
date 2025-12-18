use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::sync::RwLock;
use std::fs;
use trust_dns_resolver::TokioAsyncResolver;

use crossterm::{event, execute, terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen}};
use ratatui::{backend::CrosstermBackend, Terminal};
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph};
use ratatui::style::{Style, Color, Modifier};

use crate::cli::types::Connection;

const PORT_MAPPINGS: &[(u16, &str)] = &[
    (1, "TCPMUX"),
    (5, "RJE"),
    (7, "ECHO"),
    (18, "MSP"),
    (20, "FTP-data"),
    (21, "FTP"),
    (22, "SSH"),
    (23, "TELNET"),
    (25, "SMTP"),
    (37, "TIME"),
    (42, "NAMESERV"),
    (43, "WHOIS"),
    (49, "TACACS"),
    (53, "DNS"),
    (69, "TFTP"),
    (70, "GOPHER"),
    (79, "FINGER"),
    (80, "HTTP"),
    (103, "X400"),
    (108, "SNA_GATEWAY"),
    (109, "POP2"),
    (110, "POP3"),
    (115, "SFTP"),
    (118, "SQLSERV"),
    (119, "NNTP"),
    (137, "NETBIOS-NS"),
    (139, "NETBIOS-SS"),
    (143, "IMAP"),
    (150, "NETBIOS-SERVICE"),
    (156, "SQLSRV"),
    (161, "SNMP"),
    (162, "SNMPTRAP"),
    (179, "BGP"),
    (190, "GACP"),
    (194, "IRC"),
    (197, "DLS"),
    (389, "LDAP"),
    (396, "NOVELL-IP"),
    (443, "HTTPS"),
    (444, "SNPP"),
    (445, "MICROSOFT-DS"),
    (458, "QUICKTIME"),
    (546, "DHCP-CLIENT"),
    (547, "DHCP-SERVER"),
    (563, "SNEWS"),
    (569, "MSN"),
    (1080, "SOCKS"),
    (2379, "ETCD"),
    (3306, "MYSQL"),
    (5432, "POSTGRES"),
    (6379, "REDIS"),
    (10250, "KUBELET"),
    (10255, "KUBELET-READ"),
];

fn build_name_index() -> std::collections::HashMap<String, Vec<u16>> {
    let mut m: std::collections::HashMap<String, Vec<u16>> = std::collections::HashMap::new();
    for (p, name) in PORT_MAPPINGS.iter() {
        let key = name.to_lowercase();
        m.entry(key).or_default().push(*p);
        for word in name.split(|c: char| !c.is_alphanumeric()) {
            if word.is_empty() { continue; }
            m.entry(word.to_lowercase()).or_default().push(*p);
        }
    }
    m
}

fn conn_matches_search(c: &Connection, term: &str, name_index: &std::collections::HashMap<String, Vec<u16>>) -> bool {
    let s = term.trim().to_lowercase();
    if s.is_empty() { return true; }

    if let Ok(pnum) = s.parse::<u16>() {
        return c.src_port == pnum || c.dst_port == pnum;
    }

    if s.contains('.') || s.contains(':') {
        return c.src_ip.contains(&s) || c.dst_ip.contains(&s);
    }

    if let Some(ports) = name_index.get(&s) {
        if ports.iter().any(|p| *p == c.src_port || *p == c.dst_port) { return true; }
    }

    let src_desc = port_reservation_info(c.src_port).to_lowercase();
    let dst_desc = port_reservation_info(c.dst_port).to_lowercase();
    src_desc.contains(&s) || dst_desc.contains(&s)
}

fn rfc1700_snippet_for_port(port: u16) -> String {
    for (p, name) in PORT_MAPPINGS.iter() {
        if *p == port {
            return format!("{} ({})", p, name);
        }
    }
    port_reservation_info(port)
}

fn format_throughput(bytes_per_sec: u64) -> String {
    if bytes_per_sec == 0 {
        return "-".to_string();
    }
    if bytes_per_sec < 1024 {
        format!("{}B/s", bytes_per_sec)
    } else if bytes_per_sec < 1024 * 1024 {
        format!("{:.1}KB/s", bytes_per_sec as f64 / 1024.0)
    } else {
        format!("{:.1}MB/s", bytes_per_sec as f64 / (1024.0 * 1024.0))
    }
}

fn port_reservation_info(port: u16) -> String {
    match port {
        1 => "TCP Port Service Multiplexer (TCPMUX)".into(),
        5 => "Remote Job Entry (RJE)".into(),
        7 => "ECHO".into(),
        18 => "Message Send Protocol (MSP)".into(),
        20 => "FTP — Data".into(),
        21 => "FTP — Control".into(),
        22 => "SSH Remote Login Protocol".into(),
        23 => "Telnet".into(),
        25 => "Simple Mail Transfer Protocol (SMTP)".into(),
        29 => "MSG ICP".into(),
        37 => "Time".into(),
        42 => "Host Name Server (Nameserv)".into(),
        43 => "WhoIs".into(),
        49 => "Login Host Protocol (Login)".into(),
        53 => "Domain Name System (DNS)".into(),
        69 => "Trivial File Transfer Protocol (TFTP)".into(),
        70 => "Gopher Services".into(),
        79 => "Finger".into(),
        80 => "HTTP".into(),
        103 => "X.400 Standard".into(),
        108 => "SNA Gateway Access Server".into(),
        109 => "POP2".into(),
        110 => "POP3".into(),
        115 => "Simple File Transfer Protocol (SFTP)".into(),
        118 => "SQL Services".into(),
        119 => "Newsgroup (NNTP)".into(),
        137 => "NetBIOS Name Service".into(),
        139 => "NetBIOS Datagram Service".into(),
        143 => "Interim Mail Access Protocol (IMAP)".into(),
        150 => "NetBIOS Session Service".into(),
        156 => "SQL Server".into(),
        161 | 162 => "SNMP".into(),
        179 => "Border Gateway Protocol (BGP)".into(),
        190 => "Gateway Access Control Protocol (GACP)".into(),
        194 => "Internet Relay Chat (IRC)".into(),
        197 => "Directory Location Service (DLS)".into(),
        389 => "Lightweight Directory Access Protocol (LDAP)".into(),
        396 => "Novell Netware over IP".into(),
        443 => "HTTPS".into(),
        444 => "Simple Network Paging Protocol (SNPP)".into(),
        445 => "Microsoft-DS".into(),
        458 => "Apple QuickTime".into(),
        546 => "DHCP Client".into(),
        547 => "DHCP Server".into(),
        563 => "SNEWS".into(),
        569 => "MSN".into(),
        1080 => "Socks".into(),
        3306 => "MySQL".into(),
        5432 => "Postgres".into(),
        6379 => "Redis".into(),
        2379 => "etcd".into(),
        10250 => "kubelet".into(),
        10255 => "kubelet(read)".into(),
        _ => {
            if port <= 1023 {
                format!("Well-known ({})", port)
            } else if port <= 49151 {
                format!("Registered ({})", port)
            } else {
                format!("Dynamic/Private ({})", port)
            }
        }
    }
}

fn load_hosts_file() -> HashMap<String, String> {
    let mut hosts = HashMap::new();
    if let Ok(content) = fs::read_to_string("/etc/hosts") {
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                let ip = parts[0];
                let hostname = parts[1];
                hosts.insert(ip.to_string(), hostname.to_string());
            }
        }
    }
    hosts
}

async fn spawn_dns_resolver(
    state: Arc<RwLock<HashMap<String, Vec<Connection>>>>,
    hosts_cache: Arc<RwLock<HashMap<String, String>>>,
) {
    let resolver = match TokioAsyncResolver::tokio_from_system_conf() {
        Ok(r) => r,
        Err(_) => return,
    };

    let mut seen_ips = HashSet::new();
    loop {
        tokio::time::sleep(Duration::from_secs(2)).await;
        
        let map = state.read().await;
        let mut ips_to_resolve = Vec::new();
        
        for conns in map.values() {
            for c in conns {
                if !seen_ips.contains(&c.src_ip) {
                    ips_to_resolve.push(c.src_ip.clone());
                    seen_ips.insert(c.src_ip.clone());
                }
                if !seen_ips.contains(&c.dst_ip) {
                    ips_to_resolve.push(c.dst_ip.clone());
                    seen_ips.insert(c.dst_ip.clone());
                }
            }
        }
        drop(map);

        for ip in ips_to_resolve {
            if let Ok(addr) = ip.parse::<std::net::IpAddr>() {
                if let Ok(lookup) = resolver.reverse_lookup(addr).await {
                    if let Some(name) = lookup.iter().next() {
                        let hostname = name.to_string().trim_end_matches('.').to_string();
                        hosts_cache.write().await.insert(ip.clone(), hostname);
                    }
                }
            }
        }
    }
}

fn truncate_ipv6(ip: &str, max_len: usize) -> String {
    if ip.contains(':') && ip.len() > max_len {
        format!("{}...", &ip[..max_len.saturating_sub(3)])
    } else {
        ip.to_string()
    }
}

fn display_ip(ip: &str, hosts: &HashMap<String, String>, show_hostnames: bool) -> String {
    if show_hostnames {
        hosts.get(ip).cloned().unwrap_or_else(|| truncate_ipv6(ip, 18))
    } else {
        truncate_ipv6(ip, 18)
    }
}

pub async fn run_tui(state: Arc<RwLock<HashMap<String, Vec<Connection>>>>, kube_mode: bool, did_fetch_once: Arc<AtomicBool>) -> anyhow::Result<()> {
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let refresh_interval = Duration::from_millis(1000);
    let mut last_refresh = tokio::time::Instant::now();

    enum Focus { Nodes, Shared, Connections }
    enum SortMode { None, ByState }
    enum FilterMode { None, Established, TimeWait }
    enum InputMode { Normal, Searching }
    enum IpVersionFilter { Both, Ipv4Only, Ipv6Only }

    let hosts_cache = Arc::new(RwLock::new(load_hosts_file()));
    let mut show_hostnames = false;
    let mut ip_version_filter = IpVersionFilter::Both;
    
    tokio::spawn(spawn_dns_resolver(state.clone(), hosts_cache.clone()));
    
    let mut selected = 0usize;
    let mut show_details = false;
    let mut focus = Focus::Nodes;
    let mut conn_selected = 0usize;
    let mut sort_mode = SortMode::None;
    let mut shared_selected = 0usize;
    let mut pair_filter: Option<(String, String)> = None;
    let mut filter_mode = FilterMode::None;
    let mut input_mode = InputMode::Normal;
    let mut search_buffer = String::new();
    let mut search_term: Option<String> = None;
    let name_index = build_name_index();

    let mut modal_dismissed = false;
    let mut help_modal = false;
    loop {
        let map = state.read().await.clone();
        let mut nodes: Vec<_> = map.keys().cloned().collect();
        nodes.sort();

        let mut ip_sets: HashMap<String, HashSet<String>> = HashMap::new();
        for (node, conns) in &map {
            let mut s = HashSet::new();
            for c in conns {
                s.insert(c.src_ip.clone());
                s.insert(c.dst_ip.clone());
            }
            ip_sets.insert(node.clone(), s);
        }

        let mut ip_to_nodes: HashMap<String, Vec<String>> = HashMap::new();
        for (node, ips) in &ip_sets {
            for ip in ips {
                ip_to_nodes.entry(ip.clone()).or_default().push(node.clone());
            }
        }

        let mut edges: HashMap<(String, String), (usize, String)> = HashMap::new();
        for (node, conns) in &map {
            for c in conns {
                for (other_node, other_ips) in &ip_sets {
                    if other_node == node { continue; }
                    if other_ips.contains(&c.src_ip) || other_ips.contains(&c.dst_ip) {
                        let a = node;
                        let b = other_node;
                        let key = if a <= b { (a.clone(), b.clone()) } else { (b.clone(), a.clone()) };
                        let sample_ip = if other_ips.contains(&c.src_ip) { c.src_ip.clone() } else { c.dst_ip.clone() };
                        let entry = edges.entry(key).or_insert((0usize, sample_ip));
                        entry.0 += 1;
                    }
                }
            }
        }

        let mut edge_list: Vec<((String, String), (usize, String))> = edges.into_iter().collect();
        edge_list.sort_by(|a, b| b.1 .0.cmp(&a.1 .0));

        let hosts = hosts_cache.read().await.clone();

        terminal.draw(|f| {
            let size = f.area();
            let outer = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(3), Constraint::Length(3)].as_ref())
                .split(size);
            let top = outer[0];
            let status_area = outer[1];

            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(20), Constraint::Percentage(80)].as_ref())
                .split(top);

            let right_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Percentage(35), Constraint::Percentage(65)].as_ref())
                .split(chunks[1]);

            let items: Vec<ListItem> = nodes.iter().map(|n| {
                let c = map.get(n).map(|v| v.len()).unwrap_or(0);
                let name = if n.len() > 15 {
                    format!("{:12}...", &n[..12])
                } else {
                    n.to_string()
                };
                ListItem::new(format!("{} ({})", name, c))
            }).collect();
            let mut stateful = ratatui::widgets::ListState::default();
            if !nodes.is_empty() {
                stateful.select(Some(selected.min(nodes.len()-1)));
            }
            let list = List::new(items)
                .block(Block::default().borders(Borders::ALL).title("Nodes"))
                .highlight_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
                .highlight_symbol("» ");
            f.render_stateful_widget(list, chunks[0], &mut stateful);

            let shared_items: Vec<ListItem> = edge_list.iter().map(|((a,b),(count,ip))| {
                ListItem::new(format!("{} <-> {} ({}) [{}]", a, b, count, ip))
            }).collect();
            let mut shared_state = ratatui::widgets::ListState::default();
            if !shared_items.is_empty() {
                shared_state.select(Some(shared_selected.min(shared_items.len()-1)));
            }
            let shared_list = List::new(shared_items)
                .block(Block::default().borders(Borders::ALL).title("Shared"))
                .highlight_style(Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD))
                .highlight_symbol("» ");
            f.render_stateful_widget(shared_list, right_chunks[0], &mut shared_state);

            let selected_node = nodes.get(selected).cloned();
            if show_details {
                let mut conns: Vec<Connection> = Vec::new();
                if let Some((ref a, ref b)) = pair_filter {
                    if let Some(list_a) = map.get(a) {
                        if let Some(other_ips) = ip_sets.get(b) {
                            for c in list_a.iter() {
                                if other_ips.contains(&c.src_ip) || other_ips.contains(&c.dst_ip) {
                                    conns.push(c.clone());
                                }
                            }
                        }
                    }
                    if let Some(list_b) = map.get(b) {
                        if let Some(other_ips) = ip_sets.get(a) {
                            for c in list_b.iter() {
                                if other_ips.contains(&c.src_ip) || other_ips.contains(&c.dst_ip) {
                                    conns.push(c.clone());
                                }
                            }
                        }
                    }
                } else if let Some(node) = selected_node {
                    conns = map.get(&node).cloned().unwrap_or_default();
                }

                if let Some(ref term) = search_term {
                    conns.retain(|c| conn_matches_search(c, term, &name_index));
                }

                match filter_mode {
                    FilterMode::Established => {
                        conns.retain(|c| c.state.eq_ignore_ascii_case("ESTABLISHED"));
                    }
                    FilterMode::TimeWait => {
                        conns.retain(|c| c.state.eq_ignore_ascii_case("TIME_WAIT") || c.state.eq_ignore_ascii_case("TIME-WAIT"));
                    }
                    FilterMode::None => {}
                }

                match ip_version_filter {
                    IpVersionFilter::Ipv4Only => {
                        conns.retain(|c| !c.src_ip.contains(':') && !c.dst_ip.contains(':'));
                    }
                    IpVersionFilter::Ipv6Only => {
                        conns.retain(|c| c.src_ip.contains(':') || c.dst_ip.contains(':'));
                    }
                    IpVersionFilter::Both => {}
                }

                // Default: rank by throughput descending unless explicitly sorting by state
                match sort_mode {
                    SortMode::ByState => {
                        conns.sort_by(|a, b| a.state.cmp(&b.state));
                    }
                    SortMode::None => {
                        conns.sort_by(|a, b| b.throughput_bytes_per_sec.cmp(&a.throughput_bytes_per_sec));
                    }
                }

                let items: Vec<ListItem> = conns.iter().map(|c| {
                    let src_ip_str = display_ip(&c.src_ip.to_string(), &hosts, show_hostnames);
                    let dst_ip_str = display_ip(&c.dst_ip.to_string(), &hosts, show_hostnames);
                    let src = format!("{}:{}", src_ip_str, c.src_port);
                    let dst = format!("{}:{}", dst_ip_str, c.dst_port);
                    let port_info = port_reservation_info(c.dst_port);
                    let throughput = format_throughput(c.throughput_bytes_per_sec);
                    let line = format!("{:<6} {:<22} {:<22} {:<12} {:<12} {:<20}", c.proto, src, dst, c.state, throughput, port_info);
                    ListItem::new(line)
                }).collect();

                if !items.is_empty() {
                    let mut list_state = ratatui::widgets::ListState::default();
                    if conn_selected >= items.len() { conn_selected = items.len() - 1; }
                    list_state.select(Some(conn_selected));

                    let title = match sort_mode {
                        SortMode::None => "Connections (ranked by throughput)",
                        SortMode::ByState => "Connections (sorted by state)",
                    };

                    let conn_chunks = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints([Constraint::Percentage(75), Constraint::Percentage(25)].as_ref())
                        .split(right_chunks[1]);

                    let list = List::new(items)
                        .block(Block::default().borders(Borders::ALL).title(title))
                        .highlight_style(Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))
                        .highlight_symbol("» ");
                    f.render_stateful_widget(list, conn_chunks[0], &mut list_state);

                    let snippet = {
                        let idx = conn_selected.min(conns.len().saturating_sub(1));
                        if let Some(c) = conns.get(idx) {
                            let p = c.dst_port;
                            rfc1700_snippet_for_port(p)
                        } else {
                            "".to_string()
                        }
                    };
                    let details = if snippet.is_empty() {
                        Paragraph::new("(no details)")
                            .block(Block::default().borders(Borders::ALL).title("Details"))
                    } else {
                        Paragraph::new(snippet)
                            .block(Block::default().borders(Borders::ALL).title("Port Info"))
                    };
                    f.render_widget(details, conn_chunks[1]);
                } else {
                    let conn_chunks = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints([Constraint::Percentage(100)].as_ref())
                        .split(right_chunks[1]);
                    let paragraph = Paragraph::new("(no node selected)")
                        .block(Block::default().borders(Borders::ALL).title("Connections"));
                    f.render_widget(paragraph, conn_chunks[0]);
                }
            } else {
                let paragraph = Paragraph::new("Press Right or Enter to view connections for the selected node. Use Up/Down to choose a node. Press 't' to toggle sort by state. Press 'f' to cycle state filter. Press 'p' to search by IP. Press 'q' to quit.")
                    .block(Block::default().borders(Borders::ALL).title("Connections"));
                f.render_widget(paragraph, right_chunks[1]);
            }
            let focus_str = match focus { Focus::Nodes => "Nodes", Focus::Shared => "Shared", Focus::Connections => "Connections" };
            let focus_color = match focus { Focus::Nodes => Color::Cyan, Focus::Shared => Color::Magenta, Focus::Connections => Color::Green };
            let ip_filter_str = match ip_version_filter { IpVersionFilter::Both => "both", IpVersionFilter::Ipv4Only => "IPv4", IpVersionFilter::Ipv6Only => "IPv6" };
            let status = format!("Focus: {} | Filter: {} | Search: {} | Names: {} | IP: {}{}",
                focus_str,
                match filter_mode { FilterMode::None => "none".to_string(), FilterMode::Established => "ESTABLISHED".to_string(), FilterMode::TimeWait => "TIME_WAIT".to_string(), },
                search_term.as_deref().unwrap_or("<none>"),
                if show_hostnames { "ON" } else { "OFF" },
                ip_filter_str,
                match input_mode {
                    InputMode::Searching => format!(" | typing: {}", search_buffer),
                    _ => "".to_string(),
                }
            );
            let sb = Paragraph::new(status)
                .block(Block::default().borders(Borders::ALL).title("Status"))
                .style(Style::default().fg(focus_color));
            f.render_widget(sb, status_area);

            let modal_visible = kube_mode && nodes.is_empty() && did_fetch_once.load(Ordering::SeqCst) && !modal_dismissed;
                if help_modal {
                let mw = (size.width.saturating_mul(70)) / 100;
                let mh = (size.height.saturating_mul(70)) / 100;
                let mx = size.x + (size.width.saturating_sub(mw)) / 2;
                let my = size.y + (size.height.saturating_sub(mh)) / 2;
                let area = ratatui::layout::Rect::new(mx, my, mw, mh);
                let help_text = "Key bindings:\n\nUp/Down: move selection\nLeft/Right or Tab: change focus pane\nEnter: open connections / toggle details\nq: quit\np: start search (type term, Enter to apply, Esc to cancel)\nEsc: cancel typing / dismiss modal\nt: toggle sort by state\nf: cycle state filter (none -> ESTABLISHED -> TIME_WAIT)\nc: clear pair-filter\nn: toggle hostnames / IPs\nv: cycle IP version filter (both -> IPv4 -> IPv6)\nh: show this help\n\nPress Enter, Esc, or 'h' to close.";
                f.render_widget(Clear, area);
                let p = Paragraph::new(help_text)
                    .block(Block::default().borders(Borders::ALL).title("kflow — Help"))
                    .style(Style::default().fg(Color::White).bg(Color::Black));
                f.render_widget(p, area);
            } else if modal_visible {
                let mw = (size.width.saturating_mul(60)) / 100;
                let mh = (size.height.saturating_mul(30)) / 100;
                let mx = size.x + (size.width.saturating_sub(mw)) / 2;
                let my = size.y + (size.height.saturating_sub(mh)) / 2;
                let area = ratatui::layout::Rect::new(mx, my, mw, mh);
                let text = "No kflow-daemon pods found. Run `kflow install -n <ns>` or apply the provided k8s/daemonset.yaml. Press Enter or Esc to dismiss.";
                let p = Paragraph::new(text)
                    .block(Block::default().borders(Borders::ALL).title("No Daemon Pods"))
                    .style(Style::default().fg(Color::Red));
                f.render_widget(p, area);
            }
        })?;

        let conn_count = if let Some(node) = state.read().await.keys().cloned().collect::<Vec<_>>().get(selected) {
            let mut c = state.read().await.get(node).cloned().unwrap_or_default();
            if let Some(ref term) = search_term { c.retain(|x| conn_matches_search(x, term, &name_index)); }
            match filter_mode {
                FilterMode::Established => c.retain(|x| x.state.eq_ignore_ascii_case("ESTABLISHED")),
                FilterMode::TimeWait => c.retain(|x| x.state.eq_ignore_ascii_case("TIME_WAIT") || x.state.eq_ignore_ascii_case("TIME-WAIT")),
                FilterMode::None => {}
            }
            match sort_mode {
                SortMode::ByState => c.sort_by(|a,b| a.state.cmp(&b.state)),
                SortMode::None => c.sort_by(|a,b| b.throughput_bytes_per_sec.cmp(&a.throughput_bytes_per_sec)),
            }
            c.len()
        } else { 0 };

        let timeout = refresh_interval.checked_sub(last_refresh.elapsed()).unwrap_or_default();
        if event::poll(timeout)? {
            if let event::Event::Key(key) = event::read()? {
                // Prioritize help modal dismissal if it's visible
                if help_modal {
                    match key.code {
                        event::KeyCode::Enter | event::KeyCode::Esc | event::KeyCode::Char('h') => {
                            help_modal = false;
                        }
                        _ => {}
                    }
                    continue;
                }

                let modal_visible = kube_mode && nodes.is_empty() && !modal_dismissed;
                if modal_visible {
                    match key.code {
                        event::KeyCode::Enter | event::KeyCode::Esc | event::KeyCode::Char('c') => {
                            modal_dismissed = true;
                        }
                        _ => {}
                    }
                    continue;
                }

                if let InputMode::Searching = input_mode {
                    match key.code {
                        event::KeyCode::Esc => {
                            input_mode = InputMode::Normal;
                            search_buffer.clear();
                        }
                        event::KeyCode::Enter => {
                            if !search_buffer.trim().is_empty() {
                                search_term = Some(search_buffer.trim().to_string());
                            } else {
                                search_term = None;
                            }
                            input_mode = InputMode::Normal;
                        }
                        event::KeyCode::Backspace => { search_buffer.pop(); }
                        event::KeyCode::Char(c) => { search_buffer.push(c); }
                        _ => {}
                    }
                } else {
                    match key.code {
                        event::KeyCode::Char('q') => break,
                        event::KeyCode::Down => {
                            if let Focus::Nodes = focus {
                                if !state.read().await.is_empty() {
                                    selected = selected.saturating_add(1).min(state.read().await.len().saturating_sub(1));
                                    show_details = false;
                                    conn_selected = 0;
                                }
                            } else if let Focus::Shared = focus {
                                if !edge_list.is_empty() {
                                    shared_selected = shared_selected.saturating_add(1).min(edge_list.len().saturating_sub(1));
                                }
                            } else {
                                if conn_count > 0 {
                                    conn_selected = conn_selected.saturating_add(1).min(conn_count.saturating_sub(1));
                                }
                            }
                        }
                        event::KeyCode::Up => {
                            if let Focus::Nodes = focus {
                                if selected > 0 { selected -= 1; }
                                show_details = false;
                                conn_selected = 0;
                            } else if let Focus::Shared = focus {
                                if shared_selected > 0 { shared_selected -= 1; }
                            } else {
                                if conn_selected > 0 { conn_selected -= 1; }
                            }
                        }
                        event::KeyCode::Enter => {
                            if let Focus::Shared = focus {
                                if !edge_list.is_empty() && shared_selected < edge_list.len() {
                                    let ((a,b), _) = &edge_list[shared_selected];
                                    pair_filter = Some((a.clone(), b.clone()));
                                    show_details = true;
                                    focus = Focus::Connections;
                                    conn_selected = 0;
                                }
                            } else if let Focus::Nodes = focus {
                                pair_filter = None;
                                show_details = true;
                                focus = Focus::Connections;
                                conn_selected = 0;
                            } else {
                                show_details = !show_details;
                                if show_details { focus = Focus::Connections; } else { focus = Focus::Nodes; }
                            }
                        }
                        event::KeyCode::Right | event::KeyCode::Tab => {
                            let prev = match focus { Focus::Nodes => Focus::Nodes, Focus::Shared => Focus::Shared, Focus::Connections => Focus::Connections };
                            focus = match focus {
                                Focus::Nodes => Focus::Shared,
                                Focus::Shared => Focus::Connections,
                                Focus::Connections => Focus::Nodes,
                            };
                            if let Focus::Connections = focus {
                                if !show_details { show_details = true; }
                                if let Focus::Nodes = prev { pair_filter = None; }
                            }
                        }
                        event::KeyCode::Left => {
                            let prev = match focus { Focus::Nodes => Focus::Nodes, Focus::Shared => Focus::Shared, Focus::Connections => Focus::Connections };
                            focus = match focus {
                                Focus::Nodes => Focus::Connections,
                                Focus::Shared => Focus::Nodes,
                                Focus::Connections => Focus::Shared,
                            };
                            if let Focus::Connections = focus { if let Focus::Nodes = prev { pair_filter = None; } }
                        }
                        event::KeyCode::Char('r') => {}
                        event::KeyCode::Char('t') => {
                            sort_mode = match sort_mode {
                                SortMode::None => SortMode::ByState,
                                SortMode::ByState => SortMode::None,
                            };
                            conn_selected = 0;
                        }
                        event::KeyCode::Char('f') => {
                            filter_mode = match filter_mode {
                                FilterMode::None => FilterMode::Established,
                                FilterMode::Established => FilterMode::TimeWait,
                                FilterMode::TimeWait => FilterMode::None,
                            };
                            conn_selected = 0;
                        }
                        event::KeyCode::Char('p') => {
                            input_mode = InputMode::Searching;
                            search_buffer.clear();
                        }
                        event::KeyCode::Char('h') => {
                            help_modal = true;
                        }
                        event::KeyCode::Char('c') => {
                            pair_filter = None;
                        }
                        event::KeyCode::Char('n') => {
                            show_hostnames = !show_hostnames;
                        }
                        event::KeyCode::Char('v') => {
                            ip_version_filter = match ip_version_filter {
                                IpVersionFilter::Both => IpVersionFilter::Ipv4Only,
                                IpVersionFilter::Ipv4Only => IpVersionFilter::Ipv6Only,
                                IpVersionFilter::Ipv6Only => IpVersionFilter::Both,
                            };
                        }
                        _ => {}
                    }
                }
            }
        }

        if last_refresh.elapsed() >= refresh_interval {
            last_refresh = tokio::time::Instant::now();
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}
