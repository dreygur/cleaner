const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];

/// Renders a byte count in decimal units (1000-based), matching the figures
/// macOS itself reports for disk usage.
pub fn human_bytes(n: u64) -> String {
    if n < 1000 {
        return format!("{n} B");
    }
    let mut value = n as f64;
    let mut unit = 0;
    while value >= 1000.0 && unit < UNITS.len() - 1 {
        value /= 1000.0;
        unit += 1;
    }
    format!("{value:.1} {}", UNITS[unit])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytes_under_1000_have_no_unit_suffix_conversion() {
        assert_eq!(human_bytes(0), "0 B");
        assert_eq!(human_bytes(999), "999 B");
    }

    #[test]
    fn crosses_into_kilobytes_at_1000() {
        assert_eq!(human_bytes(1_000), "1.0 KB");
        assert_eq!(human_bytes(1_500), "1.5 KB");
    }

    #[test]
    fn crosses_into_megabytes_gigabytes_terabytes() {
        assert_eq!(human_bytes(8_400_000_000), "8.4 GB");
        assert_eq!(human_bytes(420_000_000), "420.0 MB");
        assert_eq!(human_bytes(1_000_000_000_000), "1.0 TB");
    }
}
