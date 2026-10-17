/// Progress reported by long-running operations (downloads, extraction).
#[derive(Debug, Clone, PartialEq)]
pub enum Progress {
    /// A new phase started, e.g. "Downloading Java 21".
    Phase(String),
    /// Bytes transferred so far; `total` is unknown when the server sent no length.
    Bytes { downloaded: u64, total: Option<u64> },
}

impl Progress {
    pub fn phase(name: impl Into<String>) -> Self {
        Self::Phase(name.into())
    }

    /// Fraction in `0.0..=1.0` when the total is known.
    pub fn fraction(&self) -> Option<f32> {
        match self {
            Self::Bytes { downloaded, total: Some(total) } if *total > 0 => Some((*downloaded as f64 / *total as f64).min(1.0) as f32),
            _ => None,
        }
    }
}

/// Human-readable byte count, e.g. `31.2 MB`.
pub fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1000.0 && unit < UNITS.len() - 1 {
        value /= 1000.0;
        unit += 1;
    }
    if unit == 0 { format!("{bytes} B") } else { format!("{value:.1} {}", UNITS[unit]) }
}

pub type ProgressSender = sipper::Sender<Progress>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_bytes() {
        assert_eq!(format_bytes(999), "999 B");
        assert_eq!(format_bytes(31_200_000), "31.2 MB");
        assert_eq!(Progress::Bytes { downloaded: 50, total: Some(200) }.fraction(), Some(0.25));
        assert_eq!(Progress::Bytes { downloaded: 50, total: None }.fraction(), None);
    }
}
