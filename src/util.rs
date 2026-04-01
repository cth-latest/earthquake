use crate::Result;
use crate::proxy::Proxy;
use chrono::Local;
use reqwest::{Client, ClientBuilder};
use std::collections::HashMap;
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::Path;

pub fn create_directory_if_not_exists(path: impl AsRef<Path>) -> std::io::Result<()> {
    let path = path.as_ref();

    if !path.exists() {
        fs::create_dir_all(path)?;
    }

    Ok(())
}

pub fn format_datetime_now() -> String {
    Local::now().format("%Y-%m-%d_%H-%M-%S").to_string()
}

pub fn format_results_path(base_dir: &str, result_type: &str) -> String {
    format!("{}/{}.txt", base_dir, result_type)
}

pub async fn build_http_client(proxy: Option<&Proxy>) -> Result<Client> {
    let mut client_builder = ClientBuilder::new()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.124 Safari/537.36")
        .cookie_store(true)
        .tcp_keepalive(std::time::Duration::from_secs(30))
        .pool_idle_timeout(std::time::Duration::from_secs(60))
        .timeout(std::time::Duration::from_secs(60));

    if let Some(proxy) = proxy {
        client_builder = client_builder.proxy(proxy.to_reqwest_proxy()?);
    }

    Ok(client_builder.build()?)
}

pub fn random_string(length: usize) -> String {
    use rand::{Rng, distributions::Alphanumeric};
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(length)
        .map(char::from)
        .collect()
}

pub fn save_to_file(path: impl AsRef<Path>, content: &str) -> std::io::Result<()> {
    let path = path.as_ref();

    if let Some(parent) = path.parent() {
        create_directory_if_not_exists(parent)?;
    }

    fs::write(path, content)
}

pub fn append_to_file(path: impl AsRef<Path>, content: &str) -> std::io::Result<()> {
    let path = path.as_ref();

    if let Some(parent) = path.parent() {
        create_directory_if_not_exists(parent)?;
    }

    use std::fs::OpenOptions;

    let mut file = OpenOptions::new().create(true).append(true).open(path)?;

    file.write_all(content.as_bytes())?;
    file.write_all(b"\n")?;

    Ok(())
}

pub fn extract_captures_from_file(
    path: impl AsRef<Path>,
    capture_key: &str,
) -> io::Result<Vec<(String, String)>> {
    let file = fs::File::open(path)?;
    let reader = io::BufReader::new(file);
    let mut results = Vec::new();

    for line in reader.lines() {
        let line = line?;

        if line.contains(&format!("{}: ", capture_key)) {
            if let Some(combo) = line.split('|').next() {
                let capture_format = format!("{}: ", capture_key);
                if let Some(capture_start) = line.find(&capture_format) {
                    let start_pos = capture_start + capture_format.len();
                    let end_pos = line[start_pos..]
                        .find(" - ")
                        .map_or(line.len(), |pos| start_pos + pos);
                    let value = &line[start_pos..end_pos];
                    results.push((combo.trim().to_string(), value.to_string()));
                }
            }
        }
    }

    Ok(results)
}

pub fn parse_captures_from_line(line: &str) -> HashMap<String, String> {
    let mut captures = HashMap::new();

    let parts: Vec<&str> = line.split('|').collect();
    if parts.len() <= 1 {
        return captures;
    }

    for part in &parts[1..] {
        let capture_parts: Vec<&str> = part.split(" - ").collect();

        for capture in capture_parts {
            if let Some(colon_pos) = capture.find(':') {
                let key = capture[..colon_pos].trim();
                let value = capture[colon_pos + 1..].trim();

                if !key.is_empty() && !value.is_empty() {
                    captures.insert(key.to_string(), value.to_string());
                }
            }
        }
    }

    captures
}
