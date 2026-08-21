//! Opt-in, low-overhead performance telemetry for the native design pipeline.

use std::time::{Duration, Instant};

use serde::Serialize;

pub(crate) const PROFILE_SCHEMA: &str = "kicad_cruncher.design_review.performance_profile.a8";

#[derive(Clone, Debug, Serialize)]
pub(crate) struct PerformanceStage {
    pub name: &'static str,
    pub elapsed_ns: u64,
    pub accounted_ns: u64,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct PerformanceDetail {
    pub parent: &'static str,
    pub name: &'static str,
    pub elapsed_ns: u64,
    pub accounted_ns: u64,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct PerformanceProfile {
    pub schema: &'static str,
    pub total_elapsed_ns: u64,
    pub accounted_elapsed_ns: u64,
    pub unattributed_elapsed_ns: u64,
    pub artifact_count: usize,
    pub artifact_bytes: usize,
    pub stages: Vec<PerformanceStage>,
    pub details: Vec<PerformanceDetail>,
}

pub(crate) struct PerformanceRecorder {
    enabled: bool,
    total_started: Option<Instant>,
    stages: Vec<PerformanceStage>,
    details: Vec<PerformanceDetail>,
}

impl PerformanceRecorder {
    pub(crate) fn new(enabled: bool) -> Self {
        Self {
            enabled,
            total_started: enabled.then(Instant::now),
            stages: Vec::new(),
            details: Vec::new(),
        }
    }

    pub(crate) fn start(&self) -> Option<Instant> {
        self.enabled.then(Instant::now)
    }

    pub(crate) fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub(crate) fn finish(&mut self, name: &'static str, started: Option<Instant>) {
        if let Some(started) = started {
            let elapsed_ns = duration_ns(started.elapsed());
            self.stages.push(PerformanceStage {
                name,
                elapsed_ns,
                accounted_ns: elapsed_ns,
            });
        }
    }

    pub(crate) fn record_overlapped_stage(
        &mut self,
        name: &'static str,
        elapsed: Duration,
        blocking: Duration,
    ) -> (u64, u64) {
        let elapsed_ns = duration_ns(elapsed);
        let accounted_ns = duration_ns(blocking.min(elapsed));
        if self.enabled {
            self.stages.push(PerformanceStage {
                name,
                elapsed_ns,
                accounted_ns,
            });
        }
        (elapsed_ns, accounted_ns)
    }

    pub(crate) fn finish_detail(
        &mut self,
        parent: &'static str,
        name: &'static str,
        started: Option<Instant>,
    ) {
        if let Some(started) = started {
            let elapsed_ns = duration_ns(started.elapsed());
            self.details.push(PerformanceDetail {
                parent,
                name,
                elapsed_ns,
                accounted_ns: elapsed_ns,
            });
        }
    }

    pub(crate) fn elapsed(&self, started: Option<Instant>) -> Duration {
        started.map_or(Duration::ZERO, |started| started.elapsed())
    }

    pub(crate) fn record_detail(
        &mut self,
        parent: &'static str,
        name: &'static str,
        duration: Duration,
    ) {
        if self.enabled {
            self.details.push(PerformanceDetail {
                parent,
                name,
                elapsed_ns: duration_ns(duration),
                accounted_ns: duration_ns(duration),
            });
        }
    }

    pub(crate) fn record_overlapped_detail(
        &mut self,
        parent: &'static str,
        name: &'static str,
        elapsed: Duration,
        accounted: Duration,
    ) {
        if self.enabled {
            self.details.push(PerformanceDetail {
                parent,
                name,
                elapsed_ns: duration_ns(elapsed),
                accounted_ns: duration_ns(accounted.min(elapsed)),
            });
        }
    }

    pub(crate) fn record_overlap_accounted_details(
        &mut self,
        parent: &'static str,
        details: &[(&'static str, u64, u64)],
        parent_elapsed_ns: u64,
        parent_accounted_ns: u64,
    ) {
        if !self.enabled {
            return;
        }
        let weights = details.iter().map(|detail| detail.2).collect::<Vec<_>>();
        let accounted =
            allocate_accounted_details(&weights, parent_elapsed_ns, parent_accounted_ns);
        for ((name, elapsed_ns, _), accounted_ns) in details.iter().zip(accounted) {
            self.record_overlapped_detail(
                parent,
                name,
                Duration::from_nanos(*elapsed_ns),
                Duration::from_nanos(accounted_ns),
            );
        }
    }

    pub(crate) fn complete(
        self,
        artifact_count: usize,
        artifact_bytes: usize,
    ) -> PerformanceProfile {
        let total_elapsed_ns = self
            .total_started
            .map_or(0, |started| duration_ns(started.elapsed()));
        let accounted_elapsed_ns = self
            .stages
            .iter()
            .map(|stage| stage.accounted_ns)
            .sum::<u64>();
        PerformanceProfile {
            schema: PROFILE_SCHEMA,
            total_elapsed_ns,
            accounted_elapsed_ns,
            unattributed_elapsed_ns: total_elapsed_ns.saturating_sub(accounted_elapsed_ns),
            artifact_count,
            artifact_bytes,
            stages: self.stages,
            details: self.details,
        }
    }
}

fn allocate_accounted_details(
    weights: &[u64],
    parent_elapsed_ns: u64,
    parent_accounted_ns: u64,
) -> Vec<u64> {
    let total = weights
        .iter()
        .fold(0u128, |sum, weight| sum.saturating_add(u128::from(*weight)));
    if total == 0 || parent_elapsed_ns == 0 {
        return vec![0; weights.len()];
    }
    let covered = total.min(u128::from(parent_elapsed_ns));
    let target = u128::from(parent_accounted_ns) * covered / u128::from(parent_elapsed_ns);
    let mut shares = weights
        .iter()
        .map(|weight| {
            let share = u128::from(*weight) * target / total;
            u64::try_from(share).unwrap_or(u64::MAX)
        })
        .collect::<Vec<_>>();
    let allocated = shares.iter().map(|share| u128::from(*share)).sum::<u128>();
    let mut remainder = target.saturating_sub(allocated);
    for (share, weight) in shares.iter_mut().zip(weights) {
        if remainder == 0 {
            break;
        }
        if *share < *weight {
            *share += 1;
            remainder -= 1;
        }
    }
    shares
}

fn duration_ns(duration: Duration) -> u64 {
    u64::try_from(duration.as_nanos()).unwrap_or(u64::MAX)
}
