use std::env;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::time::Instant;

#[derive(Debug, Clone, Copy)]
enum Mode {
    Stats,
    Status,
    NewOrder,
}

impl Mode {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "stats" => Ok(Self::Stats),
            "status" => Ok(Self::Status),
            "new" => Ok(Self::NewOrder),
            other => Err(format!("unsupported mode: {other}")),
        }
    }
}

#[derive(Debug)]
struct Config {
    host: String,
    port: u16,
    product_id: u16,
    iterations: usize,
    warmup: usize,
    mode: Mode,
    seed_order_id: u64,
}

impl Config {
    fn from_env() -> Result<Self, String> {
        let mut config = Self {
            host: "127.0.0.1".to_string(),
            port: 6380,
            product_id: 7,
            iterations: 1000,
            warmup: 100,
            mode: Mode::Stats,
            seed_order_id: 900_000,
        };

        let mut args = env::args().skip(1);
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--host" => config.host = next_arg(&mut args, "--host")?,
                "--port" => config.port = next_arg(&mut args, "--port")?.parse().map_err(|_| "invalid --port".to_string())?,
                "--product-id" => {
                    config.product_id = next_arg(&mut args, "--product-id")?
                        .parse()
                        .map_err(|_| "invalid --product-id".to_string())?
                }
                "--iterations" => {
                    config.iterations = next_arg(&mut args, "--iterations")?
                        .parse()
                        .map_err(|_| "invalid --iterations".to_string())?
                }
                "--warmup" => {
                    config.warmup = next_arg(&mut args, "--warmup")?
                        .parse()
                        .map_err(|_| "invalid --warmup".to_string())?
                }
                "--mode" => config.mode = Mode::parse(&next_arg(&mut args, "--mode")?)?,
                "--seed-order-id" => {
                    config.seed_order_id = next_arg(&mut args, "--seed-order-id")?
                        .parse()
                        .map_err(|_| "invalid --seed-order-id".to_string())?
                }
                "--help" | "-h" => {
                    print_usage();
                    std::process::exit(0);
                }
                other => return Err(format!("unknown argument: {other}")),
            }
        }

        Ok(config)
    }
}

fn next_arg(args: &mut impl Iterator<Item = String>, name: &str) -> Result<String, String> {
    args.next().ok_or_else(|| format!("missing value for {name}"))
}

fn print_usage() {
    println!(
        "Usage: cargo run --release --bin redis_module_latency -- [--host 127.0.0.1] [--port 6380] [--product-id 7] [--iterations 1000] [--warmup 100] [--mode stats|status|new] [--seed-order-id 900000]"
    );
}

fn encode_resp(parts: &[String]) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(format!("*{}\r\n", parts.len()).as_bytes());
    for part in parts {
        buf.extend_from_slice(format!("${}\r\n", part.len()).as_bytes());
        buf.extend_from_slice(part.as_bytes());
        buf.extend_from_slice(b"\r\n");
    }
    buf
}

fn read_resp(reader: &mut BufReader<TcpStream>) -> Result<(), String> {
    let mut line = String::new();
    line.clear();
    reader.read_line(&mut line).map_err(|e| e.to_string())?;
    if line.is_empty() {
        return Err("empty Redis reply".to_string());
    }

    match line.as_bytes()[0] as char {
        '+' | '-' | ':' => Ok(()),
        '$' => {
            let len: isize = line[1..].trim().parse().map_err(|_| "invalid bulk length".to_string())?;
            if len < 0 {
                return Ok(());
            }
            let mut body = vec![0u8; len as usize + 2];
            std::io::Read::read_exact(reader, &mut body).map_err(|e| e.to_string())?;
            Ok(())
        }
        '*' | '%' => {
            let count: usize = line[1..].trim().parse().map_err(|_| "invalid collection length".to_string())?;
            let items = if line.starts_with('%') { count * 2 } else { count };
            for _ in 0..items {
                read_resp(reader)?;
            }
            Ok(())
        }
        '_' => Ok(()),
        other => Err(format!("unsupported RESP prefix: {other}")),
    }
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    let idx = ((sorted.len() as f64) * p).ceil() as usize;
    sorted[idx.saturating_sub(1).min(sorted.len() - 1)]
}

fn summarize(samples_ms: &[f64]) {
    let mut sorted = samples_ms.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let avg = sorted.iter().sum::<f64>() / sorted.len() as f64;

    println!("count={}", sorted.len());
    println!("avg_ms={:.6}", avg);
    println!("p50_ms={:.6}", percentile(&sorted, 0.50));
    println!("p95_ms={:.6}", percentile(&sorted, 0.95));
    println!("p99_ms={:.6}", percentile(&sorted, 0.99));
    println!("min_ms={:.6}", sorted[0]);
    println!("max_ms={:.6}", sorted[sorted.len() - 1]);
}

fn make_command(config: &Config, index: usize) -> Vec<String> {
    match config.mode {
        Mode::Stats => vec![
            "fix.stats".to_string(),
            config.product_id.to_string(),
        ],
        Mode::Status => vec![
            "fix.send".to_string(),
            config.product_id.to_string(),
            format!("35=H|37={}|55=AAPL|", config.seed_order_id),
        ],
        Mode::NewOrder => vec![
            "fix.send".to_string(),
            config.product_id.to_string(),
            format!(
                "35=D|11={}|54=1|38=1|40=2|44=101|55=AAPL|",
                config.seed_order_id + index as u64 + 1
            ),
        ],
    }
}

fn prime_status_order(config: &Config, writer: &mut TcpStream, reader: &mut BufReader<TcpStream>) -> Result<(), String> {
    if !matches!(config.mode, Mode::Status) {
        return Ok(());
    }

    let reset = encode_resp(&[
        "fix.reset".to_string(),
        config.product_id.to_string(),
    ]);
    writer.write_all(&reset).map_err(|e| e.to_string())?;
    writer.flush().map_err(|e| e.to_string())?;
    read_resp(reader)?;

    let submit = encode_resp(&[
        "fix.send".to_string(),
        config.product_id.to_string(),
        format!("35=D|11={}|54=1|38=1|40=2|44=101|55=AAPL|", config.seed_order_id),
    ]);
    writer.write_all(&submit).map_err(|e| e.to_string())?;
    writer.flush().map_err(|e| e.to_string())?;
    read_resp(reader)?;
    Ok(())
}

fn run(config: Config) -> Result<(), String> {
    let stream = TcpStream::connect((config.host.as_str(), config.port)).map_err(|e| e.to_string())?;
    stream.set_nodelay(true).map_err(|e| e.to_string())?;
    let mut writer = stream.try_clone().map_err(|e| e.to_string())?;
    let mut reader = BufReader::new(stream);

    prime_status_order(&config, &mut writer, &mut reader)?;

    for i in 0..config.warmup {
        let cmd = encode_resp(&make_command(&config, i));
        writer.write_all(&cmd).map_err(|e| e.to_string())?;
        writer.flush().map_err(|e| e.to_string())?;
        read_resp(&mut reader)?;
    }

    let mut samples_ms = Vec::with_capacity(config.iterations);
    for i in 0..config.iterations {
        let cmd = encode_resp(&make_command(&config, i + config.warmup));
        let start = Instant::now();
        writer.write_all(&cmd).map_err(|e| e.to_string())?;
        writer.flush().map_err(|e| e.to_string())?;
        read_resp(&mut reader)?;
        samples_ms.push(start.elapsed().as_secs_f64() * 1000.0);
    }

    println!(
        "mode={:?} host={} port={} product_id={} iterations={} warmup={}",
        config.mode, config.host, config.port, config.product_id, config.iterations, config.warmup
    );
    summarize(&samples_ms);
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_env().map_err(std::io::Error::other)?;
    run(config).map_err(std::io::Error::other)?;
    Ok(())
}
