# mscan

A small, concurrent TCP port scanner written in Rust ("mini scan"). It tries to connect to every port in a range, in parallel, and reports which ones are **open**, **closed**, or **filtered**.

> **Use responsibly.** Only scan machines you own or have written permission to test. See [Ethics and legal note](#ethics-and-legal-note).
>
> This is an educational project built while learning Rust. For serious work, use a mature tool like [nmap](https://nmap.org/).

## Quick start

```bash
cargo install mscan
mscan 127.0.0.1                                # scan ports 1-1024 on your own machine
mscan 127.0.0.1 --ports 7995-8005 --verbose    # scan a custom range and show every port
```

## Install

Requires the [Rust toolchain](https://rustup.rs/).

**From crates.io**

```bash
cargo install mscan
```

**From source**

```bash
git clone https://github.com/precious-adeyinka/mscan
cd mscan
cargo build --release
```

The binary is at `target/release/mscan`.

## Usage

```text
mscan [OPTIONS] <IP>
```

| Option                    | Default  | Limits                                       | Description                              |
| ------------------------- | -------- | -------------------------------------------- | ---------------------------------------- |
| `<IP>`                    | required | An IP address (hostnames are not supported)  | The machine to scan                      |
| `-p, --ports <START-END>` | `1-1024` | Ports 1 to 65535, start not greater than end | Port range to scan, like `1-1024`        |
| `-t, --timeout <MS>`      | `3000`   | 1 to 5000                                    | Give up on a connection after this long  |
| `-w, --workers <N>`       | `100`    | 1 to 8000                                    | Number of ports scanned at the same time |
| `-v, --verbose`           | off      |                                              | Print every port, not just open ones     |

Invalid input is rejected before any scanning starts, with a message saying why.

### Example output

```bash
mscan 127.0.0.1
```

```text
127.0.0.1:135 OPEN (commonly msrpc)
127.0.0.1:445 OPEN (commonly smb)
2 open, 1021 closed, 1 filtered
Scanned 1024 ports in 4.42s
```

With `--verbose`, closed and filtered ports are listed too, and filtered ports show the error behind them:

```text
127.0.0.1:136 closed
127.0.0.1:137 filtered (PermissionDenied)
```

The "commonly ..." label is a guess from a small built-in table of well-known port numbers. mscan does not check what is actually running there.

### Exit codes

| Code | Meaning                                                     |
| ---- | ----------------------------------------------------------- |
| `0`  | The scan completed. Open ports do not change the exit code. |
| `2`  | Invalid arguments (bad IP, range, timeout, or workers).     |

## What the results mean

| State        | What happened                                                                                |
| ------------ | -------------------------------------------------------------------------------------------- |
| **Open**     | The TCP handshake completed, so something is listening on that port.                         |
| **Closed**   | The machine answered "connection refused". It is reachable, and nothing is listening.        |
| **Filtered** | No confirmation either way: the attempt timed out, was denied, or failed for another reason. |

Filtered is deliberately a catch-all. It can mean a firewall is dropping packets, the host is unreachable, the timeout was too short, or the operating system refused the attempt. Use `--verbose` to see which error kind was behind each filtered port.

## Known behavior: timeouts can change results

A timeout that is too short silently turns closed ports into filtered ones, and the output gives no warning.

On my Windows machine, scanning closed ports on `127.0.0.1` takes about 2 seconds each before Windows reports "refused". With `--timeout 500`, every closed port was reported as `filtered (TimedOut)`. With `--timeout 3000`, the same ports were reported as closed. That is why the default is 3000 ms.

If every port on a machine you know is up shows as filtered, suspect your timeout first. Other operating systems and remote targets will behave differently.

## How it works

1. The port range becomes a shared list, and a shared atomic counter hands out the next index.
2. Each worker thread repeatedly takes the next port, tries `connect_timeout`, and sends `(port, state)` through a channel.
3. When all workers finish, the results are sorted by port number, because threads finish in random order, and then printed.

Total time is roughly the number of rounds times the time of the slowest connection in a round:

```text
time ~ ceil(ports / workers) x time per port
```

For example, on the same Windows machine, scanning 1024 closed ports on localhost took about 22 seconds with 100 workers and about 4 seconds with 1000.

## Ethics and legal note

Port scanning without permission can be illegal in many places, and it is often against the terms of service of hosting providers and networks. Only scan machines you own, or where you have explicit written authorization.

mscan completes a full TCP handshake for every port. That is visible in the target's logs and can look like an attack to intrusion detection systems. High `--workers` values open many connections at once, which can be disruptive on real networks. Start low.

## Limitations

- TCP connect scan only. There is no UDP and no stealth (SYN) scanning.
- One IP address per run. No hostnames and no network ranges (CIDR). IPv6 addresses parse but are untested.
- No service or version detection. Names are guessed from port numbers only.
- `filtered` is ambiguous by nature, and there are no retries.
- Results are printed at the end, not as they arrive.
- Each worker is an operating system thread. The 8000 maximum is a safety rail, not a promise that your machine can handle it. If the OS cannot create a thread, the program crashes.
- No rate limiting.

## Development

```bash
cargo test
```

The tests cover the input parsers and scan a real temporary listener on `127.0.0.1` to check the open and closed states. On Windows that test takes about 2 seconds because of the closed-port delay described above.

## Roadmap

- Banner grabbing and basic service detection.
- Hostname resolution and CIDR ranges.
- Retries and rate limiting.
- Machine-readable output (JSON) and an open-ports-only mode.
- Graceful handling when the OS cannot create more threads.

## License

MIT
