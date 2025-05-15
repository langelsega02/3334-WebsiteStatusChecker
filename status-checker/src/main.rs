use std::time::{Duration, SystemTime, Instant};
use std::env;
use std::process;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;

struct WebsiteStatus {
    url: String,
    action_status: Result<u16, String>,
    response_time: Duration,
    timestamp: SystemTime,
}

#[derive(Debug)]
struct Config {
    file_path: Option<String>,
    urls: Vec<String>,
    workers: usize,
    timeout_secs: u64,
    retries: u32,
}

impl Config {
    fn from_args() -> Result<Self, String> {
        let mut args = env::args().skip(1); //skip program name

        let mut file_path = None;
        let mut urls = Vec::new();
        let mut workers = 4;
        let mut timeout_secs = 5;
        let mut retries = 0;

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--file" => {
                    file_path = Some(args.next().ok_or("Expected file path after --file")?);
                }
                "--workers" => {
                    workers = args
                        .next()
                        .ok_or("Expected number after --workers")?
                        .parse()
                        .map_err(|_| "Invalid number for --workers")?;
                }
                "--timeout" => {
                    timeout_secs = args
                        .next()
                        .ok_or("Expected number after --timeout")?
                        .parse()
                        .map_err(|_| "Invalid number for --timeout")?;
                }
                "--retries" => {
                    retries = args
                        .next()
                        .ok_or("Expected number after --retries")?
                        .parse()
                        .map_err(|_| "Invalid number for --retries")?;
                }
                _ if arg.starts_with("--") => {
                    return Err(format!("Unknown flag: {}", arg));
                }
                _ => urls.push(arg),
            }
        }

        if file_path.is_none() && urls.is_empty() {
            return Err("No URLs provided. Use --file or provide URLs directly.".to_string());
        }

        Ok(Config {
            file_path,
            urls,
            workers,
            timeout_secs,
            retries,
        })
    }
}

fn load_urls(config: &Config) -> Result<Vec<String>, String> {
    let mut urls = Vec::new();

    //load from file
    if let Some(path) = &config.file_path {
        let file = File::open(path).map_err(|e| format!("Failed to open file {}: {}", path, e))?;
        let reader = BufReader::new(file);

        for line in reader.lines() {
            let line = line.map_err(|e| format!("Error reading line: {}", e))?;
            let trimmed = line.trim();
            if !trimmed.is_empty() && !trimmed.starts_with('#') {
                urls.push(trimmed.to_string());
            }
        }
    }
    urls.extend(config.urls.clone());

    if urls.is_empty() {
        return Err("No valid URLs provided.".into());
    }

    Ok(urls)
}

fn check_website(url: &str, timeout_secs: u64, retries: u32) -> WebsiteStatus {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(timeout_secs))
        .build();

    let start = Instant::now();
    let mut result = Err("Failed to create client".to_string());

    if let Ok(client) = client {
        for attempt in 0..=retries {
            match client.get(url).send() {
                Ok(resp) => {
                    result = Ok(resp.status().as_u16());
                    break;
                }
                Err(_e) if attempt < retries => {
                    std::thread::sleep(Duration::from_millis(100));
                }
                Err(e) => {
                    result = Err(e.to_string());
                }
            }
        }
    }

    WebsiteStatus {
        url: url.to_string(),
        action_status: result,
        response_time: start.elapsed(),
        timestamp: SystemTime::now(),
    }
}

fn print_status(status: &WebsiteStatus) {
    match &status.action_status {
        Ok(code) => println!(
            "[{}] OK {} in {:?}",
            code, status.url, status.response_time
        ),
        Err(e) => println!(
            "[ERROR] {} failed: {} (after {:?})",
            status.url, e, status.response_time
        ),
    }
}

fn write_json(results: &[WebsiteStatus]) -> std::io::Result<()> {
    let mut file = File::create("status.json")?;
    writeln!(file, "[")?;

    for (i, status) in results.iter().enumerate() {
        let json = format!(
            "  {{\"url\": \"{}\", \"status\": {}, \"time_ms\": {}, \"timestamp\": \"{:?}\"}}",
            status.url,
            match &status.action_status {
                Ok(code) => code.to_string(),
                Err(e) => format!("\"{}\"", e.replace('"', "'")),
            },
            status.response_time.as_millis(),
            status.timestamp
        );

        if i + 1 < results.len() {
            writeln!(file, "{},", json)?;
        } else {
            writeln!(file, "{}", json)?;
        }
    }

    writeln!(file, "]")?;
    Ok(())
}


fn main() {
    let config = Config::from_args().unwrap_or_else(|err| {
        eprintln!("Error: {}\nUsage: website_checker [--file <path>] [URL ...] [--workers N] [--timeout S] [--retries N]", err);
        process::exit(2);
    });

    let urls = load_urls(&config).unwrap_or_else(|err| {
        println!("URL loading error: {}", err);
        std::process::exit(2);
    });

    let (tx, rx) = mpsc::channel::<String>();
    let rx = Arc::new(Mutex::new(rx));

    let (tx_status, rx_status) = mpsc::channel::<WebsiteStatus>();
    //let rx_status = Arc::new(Mutex::new(rx_status));


    let mut handles = Vec::new();

    for _ in 0..config.workers {
        let rx = Arc::clone(&rx);
        let timeout = config.timeout_secs;
        let retries = config.retries;

        let tx_status = tx_status.clone();

        let handle = thread::spawn(move || {
            loop {
                let url = {
                    let lock = rx.lock().unwrap();
                    lock.recv()
                };

                match url {
                    Ok(url) => {
                        let status = check_website(&url, timeout, retries);
                        print_status(&status);
                        tx_status.send(status).unwrap();
                    }
                    Err(_) => break, // channel closed
                }       
            }
        });

        handles.push(handle);
    }

    for url in &urls {
        if tx.send(url.clone()).is_err() {
            break;
        }
    }

    drop(tx);

    for handle in handles {
        handle.join().unwrap();
    }

    drop(tx_status);

    let mut results = Vec::new();
    while let Ok(status) = rx_status.recv() {
        results.push(status);
    }

    if let Err(e) = write_json(&results) {
        eprintln!("Failed to write status.json: {}", e);
    } else {
    println!("Successfully wrote status.json");
    }
}