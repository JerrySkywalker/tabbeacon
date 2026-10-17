//! Explicitly enabled software timing, fixed capacity, no event payload or per-key I/O.
use serde::Serialize;
use std::{
    fs::OpenOptions,
    io::{self, Write},
    path::PathBuf,
    time::Instant,
};

const CAPACITY: usize = 8192;
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Outcome {
    Pending,
    Paired,
    NoOp,
    Coalesced,
    Superseded,
    Missing,
    Error,
    Quit,
}
#[derive(Serialize)]
struct Sample {
    sequence: usize,
    event_family: &'static str,
    received_ns: u64,
    state_ns: Option<u64>,
    submitted_ns: Option<u64>,
    state_revision: u64,
    screen: &'static str,
    overlay_open: bool,
    collector_in_flight: bool,
    outcome: Outcome,
}
#[derive(Serialize)]
struct ShutdownPoint {
    stage: &'static str,
    ns: u64,
    succeeded: bool,
}
#[derive(Serialize)]
pub(super) struct Trace {
    schema: &'static str,
    input_source: &'static str,
    clock: &'static str,
    capacity: usize,
    total_received: usize,
    overflow: usize,
    samples: Vec<Sample>,
    shutdown: Vec<ShutdownPoint>,
    #[serde(skip)]
    start: Instant,
    #[serde(skip)]
    output: PathBuf,
}
impl Trace {
    pub(super) fn new(output: PathBuf) -> Self {
        Self {
            schema: "tabbeacon-software-timing-v1",
            input_source: "UNATTESTED_EVENT_STREAM_SEE_OWNED_CALLER_RECEIPT",
            clock: "std::time::Instant monotonic process-local",
            capacity: CAPACITY,
            total_received: 0,
            overflow: 0,
            samples: Vec::with_capacity(CAPACITY),
            shutdown: Vec::with_capacity(16),
            start: Instant::now(),
            output,
        }
    }
    pub(super) fn now(&self) -> u64 {
        u64::try_from(self.start.elapsed().as_nanos()).unwrap_or(u64::MAX)
    }
    pub(super) fn received(&mut self, family: &'static str) -> Option<usize> {
        self.total_received += 1;
        if self.samples.len() == self.capacity {
            self.overflow += 1;
            return None;
        }
        let index = self.samples.len();
        self.samples.push(Sample {
            sequence: self.total_received,
            event_family: family,
            received_ns: self.now(),
            state_ns: None,
            submitted_ns: None,
            state_revision: 0,
            screen: "unreduced",
            overlay_open: false,
            collector_in_flight: false,
            outcome: Outcome::Pending,
        });
        Some(index)
    }
    pub(super) fn state(
        &mut self,
        index: usize,
        revision: u64,
        screen: &'static str,
        overlay: bool,
        changed: bool,
        collector_in_flight: bool,
    ) {
        let now = self.now();
        let sample = &mut self.samples[index];
        sample.state_ns = Some(now);
        sample.state_revision = revision;
        sample.screen = screen;
        sample.overlay_open = overlay;
        sample.collector_in_flight = collector_in_flight;
        if !changed {
            sample.outcome = Outcome::NoOp;
        }
    }
    pub(super) fn state_returned_at(&mut self, index: usize, ns: u64) {
        self.samples[index].state_ns = Some(ns);
    }
    pub(super) fn finish_sample(&mut self, index: usize, outcome: Outcome) {
        let now = self.now();
        let sample = &mut self.samples[index];
        sample.outcome = outcome;
        if outcome == Outcome::Paired {
            sample.submitted_ns = Some(now);
        }
    }
    pub(super) fn shutdown(&mut self, stage: &'static str, succeeded: bool) {
        if self.shutdown.len() < 16 {
            self.shutdown.push(ShutdownPoint {
                stage,
                ns: self.now(),
                succeeded,
            });
        }
    }
    pub(super) fn write(mut self) -> io::Result<()> {
        for sample in &mut self.samples {
            if sample.outcome == Outcome::Pending {
                sample.outcome = Outcome::Missing;
            }
        }
        let mut output = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&self.output)?;
        serde_json::to_writer(&mut output, &self).map_err(io::Error::other)?;
        output.write_all(b"\n")
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_trace_preserves_all_outcomes_and_counts_overflow_without_payload() {
        let mut trace = Trace::new(PathBuf::from("not-written"));
        for outcome in [
            Outcome::Paired,
            Outcome::NoOp,
            Outcome::Coalesced,
            Outcome::Superseded,
            Outcome::Missing,
            Outcome::Error,
            Outcome::Quit,
        ] {
            let sample = trace.received("key").unwrap();
            trace.state(sample, 1, "Overview", false, true, true);
            trace.finish_sample(sample, outcome);
        }
        for _ in trace.samples.len()..CAPACITY {
            trace.received("mouse");
        }
        assert!(trace.received("key").is_none());
        assert_eq!(trace.overflow, 1);
        assert_eq!(trace.total_received, CAPACITY + 1);
        assert_eq!(trace.samples.capacity(), CAPACITY);
        assert_eq!(trace.samples[0].outcome, Outcome::Paired);
        assert!(trace.samples[0].submitted_ns.unwrap() >= trace.samples[0].state_ns.unwrap());
        let json = serde_json::to_string(&trace).unwrap();
        assert!(
            !json.contains("payload") && !json.contains("keycode") && !json.contains("not-written")
        );
    }
}
