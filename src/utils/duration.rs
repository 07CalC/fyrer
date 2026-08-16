pub trait DurationHumanReadable {
    fn to_human_readable(&self) -> String;
}

impl DurationHumanReadable for std::time::Duration {
    fn to_human_readable(&self) -> String {
        if self.as_secs() >= 3600 {
            format!(
                "{}h {}m {:.1}s",
                self.as_secs() / 3600,
                (self.as_secs() % 3600) / 60,
                self.as_secs_f64() % 60.0
            )
        } else if self.as_secs() >= 60 {
            format!("{}m {:.1}s", self.as_secs() / 60, self.as_secs_f64() % 60.0)
        } else if self.as_millis() >= 1000 {
            format!("{:.2}s", self.as_secs_f64())
        } else if self.as_micros() >= 1000 {
            format!("{:.2}ms", self.as_micros() as f64 / 1000.0)
        } else {
            format!("{}μs", self.as_micros())
        }
    }
}
