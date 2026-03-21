use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cycle {
    pub id: String,
    pub start_date: NaiveDate,
    pub end_date: Option<NaiveDate>,
    pub symptoms: Vec<String>,
}

impl Cycle {
    pub fn duration_days(&self) -> Option<i64> {
        self.end_date
            .map(|end| (end - self.start_date).num_days())
    }

    pub fn predict_next_start(cycles: &[Cycle]) -> Option<NaiveDate> {
        if cycles.is_empty() {
            return None;
        }

        let most_recent = &cycles[0];

        if cycles.len() < 2 {
            return Some(most_recent.start_date + chrono::Duration::days(28));
        }

        let lengths: Vec<i64> = cycles
            .windows(2)
            .map(|w| (w[0].start_date - w[1].start_date).num_days())
            .filter(|d| *d > 0)
            .collect();

        if lengths.is_empty() {
            return Some(most_recent.start_date + chrono::Duration::days(28));
        }

        let avg = lengths.iter().sum::<i64>() / lengths.len() as i64;
        Some(most_recent.start_date + chrono::Duration::days(avg))
    }
}
