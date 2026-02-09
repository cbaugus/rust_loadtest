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

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f64 = 0.001;

    fn assert_approx(actual: f64, expected: f64, msg: &str) {
        assert!(
            (actual - expected).abs() < EPSILON,
            "{}: expected {}, got {}",
            msg,
            expected,
            actual
        );
    }

    // --- Concurrent model tests ---

    mod concurrent {
        use super::*;

        #[test]
        fn returns_f64_max() {
            let model = LoadModel::Concurrent;
            assert_eq!(model.calculate_current_rps(0.0, 100.0), f64::MAX);
        }

        #[test]
        fn returns_f64_max_regardless_of_elapsed_time() {
            let model = LoadModel::Concurrent;
            assert_eq!(model.calculate_current_rps(500.0, 1000.0), f64::MAX);
            assert_eq!(model.calculate_current_rps(999.0, 1000.0), f64::MAX);
        }
    }

    // --- Rps model tests ---

    mod rps {
        use super::*;

        #[test]
        fn returns_constant_target_rps() {
            let model = LoadModel::Rps { target_rps: 100.0 };
            assert_approx(model.calculate_current_rps(0.0, 60.0), 100.0, "at start");
            assert_approx(model.calculate_current_rps(30.0, 60.0), 100.0, "midway");
            assert_approx(model.calculate_current_rps(59.0, 60.0), 100.0, "near end");
        }

        #[test]
        fn works_with_fractional_rps() {
            let model = LoadModel::Rps { target_rps: 0.5 };
            assert_approx(model.calculate_current_rps(10.0, 60.0), 0.5, "fractional");
        }

        #[test]
        fn works_with_high_rps() {
            let model = LoadModel::Rps {
                target_rps: 100000.0,
            };
            assert_approx(
                model.calculate_current_rps(10.0, 60.0),
                100000.0,
                "high rps",
            );
        }
    }

    // --- RampRps model tests ---

    mod ramp_rps {
        use super::*;

        fn make_model(min: f64, max: f64, secs: u64) -> LoadModel {
            LoadModel::RampRps {
                min_rps: min,
                max_rps: max,
                ramp_duration: Duration::from_secs(secs),
            }
        }

        #[test]
        fn returns_min_at_start() {
            let model = make_model(10.0, 100.0, 90);
            assert_approx(model.calculate_current_rps(0.0, 90.0), 10.0, "at start");
        }

        #[test]
        fn midpoint_of_ramp_up() {
            // Duration 90s, first 1/3 = 30s. At 15s (midpoint of ramp-up):
            // min + (max - min) * (15/30) = 10 + 90 * 0.5 = 55
            let model = make_model(10.0, 100.0, 90);
            assert_approx(
                model.calculate_current_rps(15.0, 90.0),
                55.0,
                "midpoint ramp up",
            );
        }

        #[test]
        fn returns_max_at_end_of_ramp_up() {
            // At 30s (end of first 1/3)
            let model = make_model(10.0, 100.0, 90);
            assert_approx(
                model.calculate_current_rps(30.0, 90.0),
                100.0,
                "end of ramp up",
            );
        }

        #[test]
        fn returns_max_during_sustain_phase() {
            // Middle 1/3 is 30-60s
            let model = make_model(10.0, 100.0, 90);
            assert_approx(
                model.calculate_current_rps(45.0, 90.0),
                100.0,
                "mid sustain",
            );
        }

        #[test]
        fn midpoint_of_ramp_down() {
            // Last 1/3 starts at 60s, ends at 90s. At 75s (midpoint):
            // max - (max - min) * (15/30) = 100 - 90 * 0.5 = 55
            let model = make_model(10.0, 100.0, 90);
            assert_approx(
                model.calculate_current_rps(75.0, 90.0),
                55.0,
                "midpoint ramp down",
            );
        }

        #[test]
        fn returns_min_at_end_of_ramp_down() {
            let model = make_model(10.0, 100.0, 90);
            assert_approx(
                model.calculate_current_rps(90.0, 90.0),
                10.0,
                "end of ramp down",
            );
        }

        #[test]
        fn does_not_go_below_min() {
            // Past the ramp duration
            let model = make_model(10.0, 100.0, 90);
            let rps = model.calculate_current_rps(100.0, 100.0);
            assert!(rps >= 10.0, "should not go below min, got {}", rps);
        }

        #[test]
        fn equal_min_max_returns_constant() {
            let model = make_model(50.0, 50.0, 90);
            assert_approx(model.calculate_current_rps(0.0, 90.0), 50.0, "at start");
            assert_approx(model.calculate_current_rps(45.0, 90.0), 50.0, "midway");
            assert_approx(model.calculate_current_rps(90.0, 90.0), 50.0, "at end");
        }

        #[test]
        fn zero_duration_returns_max() {
            let model = make_model(10.0, 100.0, 0);
            assert_approx(
                model.calculate_current_rps(0.0, 0.0),
                100.0,
                "zero duration",
            );
        }
    }

    // --- DailyTraffic model tests ---

    mod daily_traffic {
        use super::*;

        // Build a DailyTraffic model with a 1000s cycle for easy math.
        // Ratios: morning_ramp=0.2, peak_sustain=0.1, mid_decline=0.2,
        //         mid_sustain=0.1, evening_decline=0.2
        // Night sustain is the remaining 0.2
        fn make_model() -> LoadModel {
            LoadModel::DailyTraffic {
                min_rps: 10.0,
                mid_rps: 50.0,
                max_rps: 100.0,
                cycle_duration: Duration::from_secs(1000),
                morning_ramp_ratio: 0.2,
                peak_sustain_ratio: 0.1,
                mid_decline_ratio: 0.2,
                mid_sustain_ratio: 0.1,
                evening_decline_ratio: 0.2,
            }
        }

        // Phase boundaries for the model above (1000s cycle):
        // Phase 1: Morning ramp    0-200s   (min 10 -> max 100)
        // Phase 2: Peak sustain  200-300s   (max 100)
        // Phase 3: Mid decline   300-500s   (max 100 -> mid 50)
        // Phase 4: Mid sustain   500-600s   (mid 50)
        // Phase 5: Evening decline 600-800s (mid 50 -> min 10)
        // Phase 6: Night sustain 800-1000s  (min 10)

        #[test]
        fn phase1_morning_ramp_start() {
            let model = make_model();
            assert_approx(
                model.calculate_current_rps(0.0, 1000.0),
                10.0,
                "morning ramp start",
            );
        }

        #[test]
        fn phase1_morning_ramp_midpoint() {
            // At 100s (midpoint of 0-200): 10 + (100-10) * (100/200) = 10 + 45 = 55
            let model = make_model();
            assert_approx(
                model.calculate_current_rps(100.0, 1000.0),
                55.0,
                "morning ramp midpoint",
            );
        }

        #[test]
        fn phase2_peak_sustain() {
            let model = make_model();
            assert_approx(
                model.calculate_current_rps(250.0, 1000.0),
                100.0,
                "peak sustain",
            );
        }

        #[test]
        fn phase3_mid_decline_midpoint() {
            // Phase 3: 300-500s. At 400s (midpoint):
            // 100 + (50-100) * (100/200) = 100 - 25 = 75
            let model = make_model();
            assert_approx(
                model.calculate_current_rps(400.0, 1000.0),
                75.0,
                "mid decline midpoint",
            );
        }

        #[test]
        fn phase4_mid_sustain() {
            let model = make_model();
            assert_approx(
                model.calculate_current_rps(550.0, 1000.0),
                50.0,
                "mid sustain",
            );
        }

        #[test]
        fn phase5_evening_decline_midpoint() {
            // Phase 5: 600-800s. At 700s (midpoint):
            // 50 + (10-50) * (100/200) = 50 - 20 = 30
            let model = make_model();
            assert_approx(
                model.calculate_current_rps(700.0, 1000.0),
                30.0,
                "evening decline midpoint",
            );
        }

        #[test]
        fn phase6_night_sustain() {
            let model = make_model();
            assert_approx(
                model.calculate_current_rps(900.0, 1000.0),
                10.0,
                "night sustain",
            );
        }

        #[test]
        fn cycle_wraps_correctly() {
            // At 1100s = 100s into second cycle = morning ramp midpoint
            let model = make_model();
            assert_approx(
                model.calculate_current_rps(1100.0, 2000.0),
                55.0,
                "second cycle morning ramp midpoint",
            );
        }

        #[test]
        fn zero_cycle_duration_returns_max() {
            let model = LoadModel::DailyTraffic {
                min_rps: 10.0,
                mid_rps: 50.0,
                max_rps: 100.0,
                cycle_duration: Duration::from_secs(0),
                morning_ramp_ratio: 0.2,
                peak_sustain_ratio: 0.1,
                mid_decline_ratio: 0.2,
                mid_sustain_ratio: 0.1,
                evening_decline_ratio: 0.2,
            };
            assert_approx(
                model.calculate_current_rps(50.0, 100.0),
                100.0,
                "zero cycle duration",
            );
        }
    }
}
