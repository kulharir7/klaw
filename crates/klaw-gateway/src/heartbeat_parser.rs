/// Parse heartbeat interval string (e.g., "30m", "1h", "2h30m")
pub fn parse_heartbeat_interval(s: &str) -> Option<std::time::Duration> {
    let s = s.trim();
    let mut total_secs: u64 = 0;
    let mut num_buf = String::new();

    for c in s.chars() {
        if c.is_ascii_digit() {
            num_buf.push(c);
        } else {
            let n: u64 = num_buf.parse().ok()?;
            num_buf.clear();
            match c {
                's' => total_secs += n,
                'm' => total_secs += n * 60,
                'h' => total_secs += n * 3600,
                'd' => total_secs += n * 86400,
                _ => return None,
            }
        }
    }

    if !num_buf.is_empty() {
        total_secs += num_buf.parse::<u64>().ok()?;
    }

    if total_secs > 0 { Some(std::time::Duration::from_secs(total_secs)) } else { None }
}