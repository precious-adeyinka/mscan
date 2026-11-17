use clap::Parser;
use std::io::ErrorKind;
use std::net::{IpAddr, SocketAddr, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

/// Maximum workers
const MAX_WORKERS: usize = 8000;
/// Maximum timeout
const MAX_TIMEOUT: u64 = 5000;

#[derive(Parser)]
#[command(author, version, about = "A simple TCP port scanner")]
struct Cli {
    /// IP address to scan (only scan machines you have permission to test)
    ip: IpAddr,

    /// Port range, like 1-1024
    #[arg(short, long, default_value="1-1024", value_parser=parse_port_range)]
    ports: (u16, u16),

    /// Connection timeout in milliseconds (1-5000)
    #[arg(short, long, default_value_t = 3000, value_parser=parse_timeout)]
    timeout: u64,

    /// Print every port, not just open ones
    #[arg(short, long)]
    verbose: bool,

    /// Number of parallel workers (1-8000)
    #[arg(short, long, default_value_t = 100, value_parser=parse_workers)]
    workers: usize,
}

#[derive(Debug, PartialEq)]
enum PortState {
    Open,
    Closed,
    Filtered(ErrorKind),
}

fn service_name(port: u16) -> Option<&'static str> {
    match port {
        21 => Some("ftp"),
        22 => Some("ssh"),
        25 => Some("smtp"),
        53 => Some("dns"),
        80 | 8080 => Some("http"),
        135 => Some("msrpc"),
        139 => Some("netbios-ssn"),
        443 => Some("https"),
        445 => Some("smb"),
        3306 => Some("mysql"),
        3389 => Some("rdp"),
        _ => None,
    }
}

fn scan_port(ip: IpAddr, port: u16, timeout: Duration) -> PortState {
    let socket = SocketAddr::new(ip, port);
    match TcpStream::connect_timeout(&socket, timeout) {
        Ok(_) => PortState::Open,
        Err(e) if e.kind() == ErrorKind::ConnectionRefused => PortState::Closed,
        Err(e) => PortState::Filtered(e.kind()),
    }
}

fn parse_workers(text: &str) -> Result<usize, String> {
    let workers: usize = text.parse().map_err(|_| "workers must be whole number")?;

    if workers == 0 {
        return Err("workers should be at least 1".to_string());
    }
    if workers > MAX_WORKERS {
        return Err(format!("workers should be max {}", MAX_WORKERS));
    }
    Ok(workers)
}

fn parse_timeout(text: &str) -> Result<u64, String> {
    let timeout: u64 = text.parse().map_err(|_| "timeout must be whole number")?;

    if timeout == 0 {
        return Err("timeout should be at least 1".to_string());
    }
    if timeout > MAX_TIMEOUT {
        return Err(format!("timeout should be max {}", MAX_TIMEOUT));
    }
    Ok(timeout)
}

fn parse_port_range(text: &str) -> Result<(u16, u16), String> {
    let (start, end) = text.split_once("-").ok_or("Expected a range like 1-1024")?;
    let start: u16 = start.parse().map_err(|_| "start is not a valid port")?;
    let end: u16 = end.parse().map_err(|_| "end is not a valid port")?;

    if start == 0 {
        return Err("port zero is reserved".to_string());
    }

    if start > end {
        return Err("start must not be greater than end".to_string());
    }

    Ok((start, end))
}

fn main() {
    let cli = Cli::parse();
    let timeout = Duration::from_millis(cli.timeout);
    let ip = cli.ip;
    let (start, end) = cli.ports;
    let ports: Arc<Vec<u16>> = Arc::new((start..=end).collect());
    let next = Arc::new(AtomicUsize::new(0));
    let workers = cli.workers.min(ports.len());

    let started = Instant::now();
    let (tx, rx) = mpsc::channel();

    for _ in 0..workers {
        let ports = Arc::clone(&ports);
        let next = Arc::clone(&next);
        let tx = tx.clone();

        thread::spawn(move || {
            loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                if i >= ports.len() {
                    break;
                }
                let port = ports[i];

                let state = scan_port(ip, port, timeout);
                tx.send((port, state)).unwrap();
            }
        });
    }
    drop(tx);

    let mut results = Vec::new();
    for item in rx {
        results.push(item);
    }
    results.sort_by_key(|item| item.0);

    let (mut open, mut closed, mut filtered) = (0, 0, 0);
    for (port, state) in &results {
        match state {
            PortState::Open => {
                match service_name(*port) {
                    Some(name) => println!("{}:{} OPEN (commonly {})", ip, port, name),
                    None => println!("{}:{} OPEN", ip, port),
                }
                open += 1;
            }
            PortState::Closed => {
                if cli.verbose {
                    println!("{}:{} closed", ip, port);
                }
                closed += 1;
            }
            PortState::Filtered(kind) => {
                if cli.verbose {
                    println!("{}:{} filtered ({:?})", ip, port, kind);
                }
                filtered += 1;
            }
        }
    }

    println!("{} open, {} closed, {} filtered", open, closed, filtered);
    println!(
        "Scanned {} ports in {:.2?}",
        results.len(),
        started.elapsed()
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::net::TcpListener;

    #[test]
    fn parse_valid_range() {
        assert_eq!(parse_port_range("1-1024"), Ok((1, 1024)));
    }

    #[test]
    fn invalid_range() {
        assert_eq!(
            parse_port_range("80"),
            Err("Expected a range like 1-1024".to_string())
        );
    }

    #[test]
    fn reversed_range() {
        assert_eq!(
            parse_port_range("90-80"),
            Err("start must not be greater than end".to_string())
        );
    }

    #[test]
    fn max_range() {
        assert_eq!(
            parse_port_range("1-70000"),
            Err("end is not a valid port".to_string())
        );
    }

    #[test]
    fn parse_zero() {
        assert_eq!(
            parse_port_range("0-10"),
            Err("port zero is reserved".to_string())
        );
    }

    #[test]
    fn parse_valid_workers() {
        assert_eq!(
            parse_workers("0"),
            Err("workers should be at least 1".to_string())
        );
        assert_eq!(parse_workers("1"), Ok(1));
        assert_eq!(parse_workers("1000"), Ok(1000));
    }

    #[test]
    fn parse_invalid_workers() {
        assert_eq!(
            parse_workers("abc"),
            Err("workers must be whole number".to_string())
        );

        assert_eq!(
            parse_workers("-5"),
            Err("workers must be whole number".to_string())
        );
    }

    #[test]
    fn parse_max_workers() {
        assert_eq!(
            parse_workers("99999"),
            Err(format!("workers should be max {}", MAX_WORKERS))
        );
    }

    #[test]
    fn parse_valid_timeout() {
        assert_eq!(
            parse_timeout("0"),
            Err("timeout should be at least 1".to_string())
        );
        assert_eq!(parse_timeout("1"), Ok(1));
        assert_eq!(parse_timeout("5000"), Ok(5000));
    }

    #[test]
    fn parse_invalid_timeout() {
        assert_eq!(
            parse_timeout("abc"),
            Err("timeout must be whole number".to_string())
        );

        assert_eq!(
            parse_timeout("-5"),
            Err("timeout must be whole number".to_string())
        );
    }

    #[test]
    fn parse_max_timeout() {
        assert_eq!(
            parse_timeout("99999"),
            Err(format!("timeout should be max {}", MAX_TIMEOUT))
        );
    }

    #[test]
    fn parse_service_name() {
        assert_eq!(service_name(22), Some("ssh"));
        assert!(service_name(8000).is_none());
    }

    #[test]
    fn scan_detects_open_and_closed_port() {
        let ip: IpAddr = "127.0.0.1".parse().unwrap();
        let timeout = Duration::from_millis(3000);

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        assert_eq!(scan_port(ip, port, timeout), PortState::Open);

        drop(listener);
        assert_eq!(scan_port(ip, port, timeout), PortState::Closed);
    }
}
