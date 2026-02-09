use tokio::time::Duration;

/// Represents different load generation models for the load test.
#[derive(Debug, Clone)]
pub enum LoadModel {
    /// No specific RPS limit, just max concurrency.
    /// Requests are sent as fast as possible within the concurrency limit.
    Concurrent,

    /// Fixed RPS target.
    /// Maintains a constant request rate throughout the test.
    Rps {
        target_rps: f64,
    },

    /// Linear ramp up/down pattern.
    /// Divides the ramp_duration into thirds:
    /// - First 1/3: Ramp from min_rps to max_rps
    /// - Middle 1/3: Sustain at max_rps
    /// - Last 1/3: Ramp down from max_rps to min_rps
    RampRps {
        min_rps: f64,
        max_rps: f64,
        ramp_duration: Duration,
    },

    /// Complex daily traffic pattern simulation.
    /// Simulates realistic daily traffic with multiple phases:
    /// 1. Morning ramp-up (min to max)
    /// 2. Peak sustain (max)
    /// 3. Mid-day decline (max to mid)
    /// 4. Mid-day sustain (mid)
    /// 5. Evening decline (mid to min)
    /// 6. Night sustain (min)
    DailyTraffic {
        min_rps: f64,
        mid_rps: f64,
        max_rps: f64,
        cycle_duration: Duration,
        morning_ramp_ratio: f64,
        peak_sustain_ratio: f64,
        mid_decline_ratio: f64,
        mid_sustain_ratio: f64,
        evening_decline_ratio: f64,
    },
}

impl LoadModel {
    /// Calculates the current target RPS based on the model and elapsed time.
    ///
    /// # Arguments
    /// * `elapsed_total_secs` - Total seconds elapsed since test start
    /// * `_overall_test_duration_secs` - Total test duration in seconds (unused for some models)
    ///
    /// # Returns
    /// The target requests per second for the current point in time.
    pub fn calculate_current_rps(
        &self,
        elapsed_total_secs: f64,
        _overall_test_duration_secs: f64,
    ) -> f64 {
        match self {
            LoadModel::Concurrent => f64::MAX,
            LoadModel::Rps { target_rps } => *target_rps,
            LoadModel::RampRps {
                min_rps,
                max_rps,
                ramp_duration,
            } => Self::calculate_ramp_rps(*min_rps, *max_rps, ramp_duration, elapsed_total_secs),
            LoadModel::DailyTraffic {
                min_rps,
                mid_rps,
                max_rps,
                cycle_duration,
                morning_ramp_ratio,
                peak_sustain_ratio,
                mid_decline_ratio,
                mid_sustain_ratio,
                evening_decline_ratio,
            } => Self::calculate_daily_traffic_rps(
                *min_rps,
                *mid_rps,
                *max_rps,
                cycle_duration,
                *morning_ramp_ratio,
                *peak_sustain_ratio,
                *mid_decline_ratio,
                *mid_sustain_ratio,
                *evening_decline_ratio,
                elapsed_total_secs,
            ),
        }
    }

    fn calculate_ramp_rps(
        min_rps: f64,
        max_rps: f64,
        ramp_duration: &Duration,
        elapsed_total_secs: f64,
    ) -> f64 {
        let total_ramp_secs = ramp_duration.as_secs_f64();

        if total_ramp_secs <= 0.0 {
            return max_rps;
        }

        let one_third_duration = total_ramp_secs / 3.0;

        if elapsed_total_secs <= one_third_duration {
            // Ramp-up phase (first 1/3)
            min_rps + (max_rps - min_rps) * (elapsed_total_secs / one_third_duration)
        } else if elapsed_total_secs <= 2.0 * one_third_duration {
            // Max load phase (middle 1/3)
            max_rps
        } else {
            // Ramp-down phase (last 1/3)
            let ramp_down_elapsed = elapsed_total_secs - 2.0 * one_third_duration;
            let rps = max_rps - (max_rps - min_rps) * (ramp_down_elapsed / one_third_duration);
            rps.max(min_rps)
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn calculate_daily_traffic_rps(
        min_rps: f64,
        mid_rps: f64,
        max_rps: f64,
        cycle_duration: &Duration,
        morning_ramp_ratio: f64,
        peak_sustain_ratio: f64,
        mid_decline_ratio: f64,
        mid_sustain_ratio: f64,
        evening_decline_ratio: f64,
        elapsed_total_secs: f64,
    ) -> f64 {
        let cycle_duration_secs = cycle_duration.as_secs_f64();

        if cycle_duration_secs <= 0.0 {
            return max_rps;
        }

        let time_in_cycle = elapsed_total_secs % cycle_duration_secs;

        let morning_ramp_end = cycle_duration_secs * morning_ramp_ratio;
        let peak_sustain_end = morning_ramp_end + (cycle_duration_secs * peak_sustain_ratio);
        let mid_decline_end = peak_sustain_end + (cycle_duration_secs * mid_decline_ratio);
        let mid_sustain_end = mid_decline_end + (cycle_duration_secs * mid_sustain_ratio);
        let evening_decline_end = mid_sustain_end + (cycle_duration_secs * evening_decline_ratio);

        if time_in_cycle < morning_ramp_end {
            // Phase 1: Morning Ramp-up (min_rps to max_rps)
            Self::linear_interpolate(min_rps, max_rps, time_in_cycle, morning_ramp_end)
        } else if time_in_cycle < peak_sustain_end {
            // Phase 2: Peak Sustain (max_rps)
            max_rps
        } else if time_in_cycle < mid_decline_end {
            // Phase 3: Mid-Day Decline (max_rps to mid_rps)
            let decline_elapsed = time_in_cycle - peak_sustain_end;
            let decline_duration = mid_decline_end - peak_sustain_end;
            Self::linear_interpolate(max_rps, mid_rps, decline_elapsed, decline_duration)
        } else if time_in_cycle < mid_sustain_end {
            // Phase 4: Mid-Day Sustain (mid_rps)
            mid_rps
        } else if time_in_cycle < evening_decline_end {
            // Phase 5: Evening Decline (mid_rps to min_rps)
            let decline_elapsed = time_in_cycle - mid_sustain_end;
            let decline_duration = evening_decline_end - mid_sustain_end;
            Self::linear_interpolate(mid_rps, min_rps, decline_elapsed, decline_duration)
        } else {
            // Phase 6: Night Sustain (min_rps)
            min_rps
        }
    }

    fn linear_interpolate(from: f64, to: f64, elapsed: f64, duration: f64) -> f64 {
        if duration <= 0.0 {
            return to;
        }
        from + (to - from) * (elapsed / duration)
    }
}
