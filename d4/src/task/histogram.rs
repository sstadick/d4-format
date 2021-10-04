use super::{SimpleTask, Task, TaskPartition};
use std::{iter::Once, ops::Range};

pub struct Histogram(String, u32, u32, Range<i32>);

impl Histogram {
    pub fn with_bin_range(chrom: &str, begin: u32, end: u32, bin_range: Range<i32>) -> Self {
        Histogram(chrom.to_string(), begin, end, bin_range)
    }
}

impl SimpleTask for Histogram {
    fn new(chr: &str, start: u32, end: u32) -> Self {
        Self(chr.to_string(), start, end, 0..1000)
    }
}

pub struct Partition {
    range: (u32, u32),
    base: i32,
    histogram: Vec<u32>,
    below: u32,
    above: u32,
}

impl TaskPartition<Once<i32>> for Partition {
    type ParentType = Histogram;
    type ResultType = (u32, Vec<u32>, u32);
    fn new(left: u32, right: u32, parent: &Histogram) -> Self {
        let param = &parent.3;
        let base = param.start;
        let size = (param.end - param.start).max(0) as usize;
        Self {
            base,
            range: (left, right),
            histogram: vec![0; size],
            below: 0,
            above: 0,
        }
    }
    fn scope(&self) -> (u32, u32) {
        self.range
    }
    #[inline(always)]
    fn feed(&mut self, _: u32, value: Once<i32>) -> bool {
        self.feed_range(0, 1, value)
    }

    #[inline(always)]
    fn feed_range(&mut self, left: u32, right: u32, mut value: Once<i32>) -> bool {
        let value = value.next().unwrap();
        let offset = value - self.base;
        if offset < 0 {
            self.below += 1;
            return true;
        }
        if offset >= self.histogram.len() as i32 {
            self.above += right - left;
            return true;
        }
        self.histogram[offset as usize] += right - left;
        true
    }

    fn into_result(self) -> (u32, Vec<u32>, u32) {
        (self.below, self.histogram, self.above)
    }
}

impl Task<std::iter::Once<i32>> for Histogram {
    type Partition = Partition;
    type Output = (u32, Vec<u32>, u32);

    fn region(&self) -> (&str, u32, u32) {
        (self.0.as_ref(), self.1, self.2)
    }

    fn combine(&self, parts: &[(u32, Vec<u32>, u32)]) -> (u32, Vec<u32>, u32) {
        if parts.is_empty() {
            return (0, vec![], 0);
        }

        let mut histogram = vec![0; parts[0].1.len()];
        let mut below = 0;
        let mut above = 0;
        for (b, v, a) in parts {
            for (idx, value) in v.iter().enumerate() {
                histogram[idx] += value;
            }
            below += b;
            above += a;
        }
        (below, histogram, above)
    }
}
