use tokio::fs::File;
use tokio::io::{AsyncBufReadExt, BufReader};

pub async fn get_vmrss_mb() -> Result<f64, String> {
    let file = File::open("/proc/self/status")
        .await
        .map_err(|e| format!("Failed to open /proc/self/status: {:?}", e))?;

    let mut reader = BufReader::new(file);
    let mut line = String::new();

    while reader
        .read_line(&mut line)
        .await
        .map_err(|e| e.to_string())?
        > 0
    {
        if line.starts_with("VmRSS:") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                let kb = parts[1]
                    .parse::<f64>()
                    .map_err(|e| format!("Failed to parse VmRSS value: {:?}", e))?;
                return Ok(kb / 1024.0);
            }
        }
        line.clear();
    }
    Err("VmRSS line not found".to_string())
}
