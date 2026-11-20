//! Low battery remains latched until stable recovery is observed after torque release.

use std::time::{Duration, Instant};

#[derive(Default)]
pub struct LowBattery {
    active: bool,
    started: Option<Instant>,
    settled: Option<Instant>,
    locked: bool,
    released: bool,
    reason: &'static str,
    recovery_window: Option<(Instant, Instant)>,
}

impl LowBattery {
    pub fn active(&self) -> bool { self.active }
    pub fn locked(&self) -> bool { self.locked }
    pub fn reason(&self) -> &'static str { self.reason }
    pub fn needs_release(&self) -> bool { self.active && self.locked && !self.released }
    pub fn released(&mut self) { self.released = true; }
    pub fn stop(&mut self) { if self.active { self.locked = true; self.reason = "manual torque off"; } }

    /// Only successful sit feedback counts toward completion. A fault or six-second
    /// deadline cuts torque even if the policy cannot finish; it never re-arms.
    pub fn advance(&mut self, now: Instant, available: bool, seated: bool) {
        if !self.active || self.locked { return; }
        let start = *self.started.get_or_insert(now);
        if !available || now.duration_since(start) >= Duration::from_secs(6) {
            self.locked = true;
            self.reason = if available { "sit timed out" } else { "feedback or sitting model unavailable" };
            return;
        }
        if now.duration_since(start) >= Duration::from_secs(2) && seated {
            let since = *self.settled.get_or_insert(now);
            if now.duration_since(since) >= Duration::from_millis(500) { self.locked = true; self.reason = "sit settled"; }
        } else { self.settled = None; }
    }

    /// Observe each fresh bus voltage sample, including failures. Round to millivolts
    /// so a nominal 6.6 V is still excluded after floating-point averaging.
    pub fn recover(&mut self, now: Instant, volts: Option<f64>) -> Option<f64> {
        let valid = volts.filter(|v| v.is_finite() && (v * 1000.0).round() > 6600.0);
        let Some(volts) = valid.filter(|_| self.active && self.locked && self.released) else {
            self.recovery_window = None;
            return None;
        };
        let since = match self.recovery_window {
            Some((since, last)) if now.duration_since(last) <= Duration::from_secs(2) => since,
            _ => now,
        };
        if now.duration_since(since) >= Duration::from_secs(3) {
            *self = Self::default();
            return Some(volts);
        }
        self.recovery_window = Some((since, now));
        None
    }

    /// True only on the first valid low reading, for the one-shot warning.
    pub fn observe(&mut self, enabled: bool, volts: f64, threshold: f64) -> bool {
        if !self.active && enabled && volts.is_finite() && volts > 0.0
            && volts <= threshold
        {
            self.active = true;
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sit_requires_feedback_then_locks_and_releases_once() {
        let mut low=LowBattery::default();let now=Instant::now();
        low.observe(true,6.2,6.2);
        low.advance(now,true,true);
        low.advance(now+Duration::from_secs(2),true,true);
        assert!(!low.locked());
        low.advance(now+Duration::from_millis(2500),true,true);
        assert!(low.locked() && low.needs_release());
        low.released();assert!(!low.needs_release());
        low.advance(now+Duration::from_secs(10),true,false);
        assert!(low.locked());
    }
    #[test]
    fn fault_or_timeout_releases_without_waiting_forever() {
        for fault in [false,true] {
            let mut low=LowBattery::default();let now=Instant::now();low.observe(true,6.1,6.2);
            low.advance(now,!fault,false);
            if !fault {assert!(!low.locked());low.advance(now+Duration::from_secs(6),true,false);}
            assert!(low.locked() && low.needs_release());
        }
    }
    #[test]
    fn transient_stability_does_not_finish_the_sit() {
        let mut low=LowBattery::default();let now=Instant::now();low.observe(true,6.1,6.2);
        low.advance(now,true,false);low.advance(now+Duration::from_secs(2),true,true);
        low.advance(now+Duration::from_millis(2400),true,false);
        low.advance(now+Duration::from_millis(2600),true,true);assert!(!low.locked());
    }

    #[test]
    fn threshold_is_inclusive_and_warning_is_once_even_after_rebound() {
        let mut low = LowBattery::default();
        assert!(!low.observe(true, 7.0, 6.2));
        assert!(!low.observe(true, 6.8, 6.2));
        assert!(!low.observe(true, 6.21, 6.2));
        assert!(low.observe(true, 6.2, 6.2));
        for v in [6.1, 7.4, 6.0] { assert!(!low.observe(true, v, 6.2)); }
        assert!(low.active());
    }

    #[test]
    fn unknown_invalid_or_disabled_readings_do_not_trigger() {
        let mut low = LowBattery::default();
        for v in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert!(!low.observe(true, v, 6.2));
        }
        assert!(!low.observe(false, 6.1, 6.2));
        assert!(!low.active());
    }
}
