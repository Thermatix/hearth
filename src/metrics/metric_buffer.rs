use crate::metrics::aggregator::NodeMetrics;
use crate::ws::server::Subscription;
use std::collections::HashMap;

pub struct MetricBuffer {
    limit: usize,
    storage_1s: Vec<NodeMetrics>,
    samples_since_5s_rollup: u8,
    storage_5s: Vec<NodeMetrics>,
}

impl MetricBuffer {
    pub fn new(limit: usize) -> MetricBuffer {
        MetricBuffer {
            limit,
            storage_1s: Vec::with_capacity(limit),
            samples_since_5s_rollup: 0,
            storage_5s: Vec::with_capacity(limit),
        }
    }

    pub fn storage(&self, timeframe: Subscription) -> &Vec<NodeMetrics> {
        match timeframe {
            Subscription::OverviewOneSecond => &self.storage_1s,
            Subscription::OverviewFiveSeconds => &self.storage_5s,
        }
    }

    pub fn push(&mut self, metrics: NodeMetrics) {
        let length = self.storage_1s.len();
        if length >= self.limit {
            self.storage_1s.drain(0..(length - self.limit));
        }

        self.storage_1s.push(metrics);
        self.samples_since_5s_rollup += 1;

        // Rollup Begin
        if self.samples_since_5s_rollup == 5 {
            let metrics: Vec<NodeMetrics> = self.storage_1s.iter().cloned().rev().take(5).collect();
            let rollup = NodeMetrics::aggregate_avg(metrics.clone());
            let length = self.storage_5s.len();
            if length >= self.limit {
                self.storage_5s.drain(0..(length - self.limit));
            }
            self.storage_5s.push(rollup);
            self.samples_since_5s_rollup = 0;
        }
    }
}

pub struct MetricBufferMap {
    limit: usize,
    storage: HashMap<String, MetricBuffer>,
}

impl MetricBufferMap {
    pub fn new(limit: usize) -> MetricBufferMap {
        MetricBufferMap {
            limit,
            storage: HashMap::new(),
        }
    }

    pub fn storage(&self) -> &HashMap<String, MetricBuffer> {
        &self.storage
    }

    pub fn push(&mut self, key: &str, metrics: NodeMetrics) {
        if let Some(buffer) = self.storage.get_mut(key) {
            buffer.push(metrics);
        } else {
            let key = key.to_string();
            let mut buffer = MetricBuffer::new(self.limit);
            buffer.push(metrics);
            self.storage.insert(key, buffer);
        }
    }
}
