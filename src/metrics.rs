use std::collections::VecDeque;
use std::time::Instant;

/// A timestamped power sample.
#[derive(Clone, Copy)]
struct PowerSample {
    watts: f64,
    time: Instant,
}

/// Tracks power data and computes session/interval metrics.
pub struct PowerMetrics {
    /// All power samples for the session.
    session_samples: Vec<f64>,
    /// Power samples for the current interval.
    interval_samples: Vec<f64>,
    /// Sliding window of timestamped samples for 30-second rolling average (NP calculation).
    rolling_window: VecDeque<PowerSample>,
    /// 30-second rolling averages raised to the 4th power, accumulated for NP.
    np_fourth_powers: Vec<f64>,
    /// Running sum of session power (for fast average).
    session_sum: f64,
    /// Running sum of interval power.
    interval_sum: f64,
    /// Whether we're currently in an interval.
    pub in_interval: bool,
}

impl PowerMetrics {
    pub fn new() -> Self {
        Self {
            session_samples: Vec::new(),
            interval_samples: Vec::new(),
            rolling_window: VecDeque::new(),
            np_fourth_powers: Vec::new(),
            session_sum: 0.0,
            interval_sum: 0.0,
            in_interval: false,
        }
    }

    /// Record a new power reading (in watts).
    pub fn record(&mut self, watts: f64) {
        let now = Instant::now();

        // Session tracking
        self.session_samples.push(watts);
        self.session_sum += watts;

        // Interval tracking
        if self.in_interval {
            self.interval_samples.push(watts);
            self.interval_sum += watts;
        }

        // NP rolling window: keep samples from the last 30 seconds
        self.rolling_window
            .push_back(PowerSample { watts, time: now });
        let cutoff = now - std::time::Duration::from_secs(30);
        while self.rolling_window.front().is_some_and(|s| s.time < cutoff) {
            self.rolling_window.pop_front();
        }

        // Compute rolling 30-second average and accumulate for NP
        // Only start accumulating once we have at least 30 seconds of data
        if self.rolling_window.len() >= 2 {
            let front_time = self.rolling_window.front().unwrap().time;
            let window_duration = now.duration_since(front_time).as_secs_f64();
            if window_duration >= 29.0 {
                let avg: f64 = self.rolling_window.iter().map(|s| s.watts).sum::<f64>()
                    / self.rolling_window.len() as f64;
                self.np_fourth_powers.push(avg.powi(4));
            }
        }
    }

    /// Average power for the entire session.
    pub fn session_avg_power(&self) -> f64 {
        if self.session_samples.is_empty() {
            return 0.0;
        }
        self.session_sum / self.session_samples.len() as f64
    }

    /// Average power for the current interval.
    pub fn interval_avg_power(&self) -> f64 {
        if self.interval_samples.is_empty() {
            return 0.0;
        }
        self.interval_sum / self.interval_samples.len() as f64
    }

    /// Normalized power for the session.
    /// NP = 4th root of the mean of (30s rolling average power)^4
    pub fn normalized_power(&self) -> f64 {
        if self.np_fourth_powers.is_empty() {
            return 0.0;
        }
        let mean_fourth: f64 =
            self.np_fourth_powers.iter().sum::<f64>() / self.np_fourth_powers.len() as f64;
        mean_fourth.powf(0.25)
    }

    /// Start a new interval, resetting interval counters.
    pub fn start_interval(&mut self) {
        self.interval_samples.clear();
        self.interval_sum = 0.0;
        self.in_interval = true;
    }

    /// Stop the current interval.
    pub fn stop_interval(&mut self) {
        self.in_interval = false;
    }

    /// Number of session samples recorded.
    pub fn session_sample_count(&self) -> usize {
        self.session_samples.len()
    }

    /// Reset all data.
    pub fn reset(&mut self) {
        self.session_samples.clear();
        self.interval_samples.clear();
        self.rolling_window.clear();
        self.np_fourth_powers.clear();
        self.session_sum = 0.0;
        self.interval_sum = 0.0;
        self.in_interval = false;
    }
}
