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

    pub fn predict_next_start(cycles: &[Cycle], settings: &CycleSettings) -> Option<NaiveDate> {
        if cycles.is_empty() {
            return None;
        }

        let most_recent = &cycles[0];
        let acl = settings.average_cycle_length;

        if cycles.len() < 2 {
            return Some(most_recent.start_date + chrono::Duration::days(acl));
        }

        let lengths: Vec<i64> = cycles
            .windows(2)
            .map(|w| (w[0].start_date - w[1].start_date).num_days())
            .filter(|d| *d > 0)
            .collect();

        if lengths.is_empty() {
            return Some(most_recent.start_date + chrono::Duration::days(acl));
        }

        let avg = lengths.iter().sum::<i64>() / lengths.len() as i64;
        Some(most_recent.start_date + chrono::Duration::days(avg))
    }

    /// Standard deviation of cycle lengths in days. Returns None if < 2 cycles.
    pub fn cycle_variance(cycles: &[Cycle]) -> Option<f64> {
        let lengths: Vec<f64> = cycles
            .windows(2)
            .map(|w| (w[0].start_date - w[1].start_date).num_days() as f64)
            .filter(|d| *d > 0.0)
            .collect();

        if lengths.len() < 2 {
            return None;
        }

        let mean = lengths.iter().sum::<f64>() / lengths.len() as f64;
        let variance = lengths.iter().map(|l| (l - mean).powi(2)).sum::<f64>() / lengths.len() as f64;
        Some(variance.sqrt())
    }
}

// --- Cycle Phase Engine ---

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CyclePhase {
    Menstruation,
    Follicular,
    Ovulation,
    EarlyLuteal,
    LateLuteal,
}

impl CyclePhase {
    pub fn label(&self) -> &'static str {
        match self {
            CyclePhase::Menstruation => "Rest & Reset",
            CyclePhase::Follicular => "Energetic & Outgoing",
            CyclePhase::Ovulation => "Peak Magnetism",
            CyclePhase::EarlyLuteal => "Winding Down",
            CyclePhase::LateLuteal => "Sensitive",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            CyclePhase::Menstruation => "\u{1F319}",  // 🌙
            CyclePhase::Follicular => "\u{1F331}",    // 🌱
            CyclePhase::Ovulation => "\u{2728}",      // ✨
            CyclePhase::EarlyLuteal => "\u{1F343}",   // 🍃
            CyclePhase::LateLuteal => "\u{1F30A}",    // 🌊
        }
    }

    pub fn color_class(&self) -> &'static str {
        match self {
            CyclePhase::Menstruation => "neon-pink",
            CyclePhase::Follicular => "neon-green",
            CyclePhase::Ovulation => "neon-orange",
            CyclePhase::EarlyLuteal => "neon-purple",
            CyclePhase::LateLuteal => "neon-magenta",
        }
    }

    /// Textbook baseline scores on a 1-5 scale: (mood, energy, libido)
    pub fn baseline_scores(&self) -> (f64, f64, f64) {
        match self {
            CyclePhase::Menstruation => (2.0, 1.5, 1.5),
            CyclePhase::Follicular => (4.0, 3.5, 3.0),
            CyclePhase::Ovulation => (5.0, 5.0, 5.0),
            CyclePhase::EarlyLuteal => (3.0, 3.0, 2.5),
            CyclePhase::LateLuteal => (2.0, 2.0, 1.5),
        }
    }

    pub fn all() -> &'static [CyclePhase] {
        &[
            CyclePhase::Menstruation,
            CyclePhase::Follicular,
            CyclePhase::Ovulation,
            CyclePhase::EarlyLuteal,
            CyclePhase::LateLuteal,
        ]
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PhaseInfo {
    pub phase: CyclePhase,
    pub cycle_day: i64,
    pub days_in_phase_remaining: i64,
    pub mood: &'static str,
    pub energy: &'static str,
    pub libido: &'static str,
    pub description: &'static str,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CycleSettings {
    pub average_cycle_length: i64,
    pub average_period_duration: i64,
    pub on_birth_control: bool,
}

impl Default for CycleSettings {
    fn default() -> Self {
        Self {
            average_cycle_length: 28,
            average_period_duration: 5,
            on_birth_control: false,
        }
    }
}

/// Compute which phase the user is in today, given their last period start date and settings.
/// Returns None if today is before the LMP or past the expected cycle length.
pub fn current_phase(
    last_period_start: NaiveDate,
    today: NaiveDate,
    settings: &CycleSettings,
) -> Option<PhaseInfo> {
    let cycle_day = (today - last_period_start).num_days() + 1;
    let acl = settings.average_cycle_length;
    let pd = settings.average_period_duration;

    if cycle_day < 1 || cycle_day > acl {
        return None;
    }

    // Ovulation almost always occurs 14 days before the next period
    let ovulation_day = (acl - 14).max(pd + 1);
    let ovulation_end = (ovulation_day + 2).min(acl);
    let late_luteal_start = (acl - 4).max(ovulation_end + 1);

    let (phase, phase_end) = if cycle_day <= pd {
        (CyclePhase::Menstruation, pd)
    } else if cycle_day < ovulation_day {
        (CyclePhase::Follicular, ovulation_day - 1)
    } else if cycle_day <= ovulation_end {
        (CyclePhase::Ovulation, ovulation_end)
    } else if cycle_day < late_luteal_start {
        (CyclePhase::EarlyLuteal, late_luteal_start - 1)
    } else {
        (CyclePhase::LateLuteal, acl)
    };

    let days_remaining = phase_end - cycle_day;

    let (mood, energy, libido, description) = match &phase {
        CyclePhase::Menstruation => (
            "Introspective",
            "Low",
            "Low",
            "Hormones at their lowest. Rest, recharge, and be gentle with yourself.",
        ),
        CyclePhase::Follicular => (
            "Upbeat & Sociable",
            "Rising",
            "Warming Up",
            "Estrogen climbing steadily. Great time for new plans, workouts, and socializing.",
        ),
        CyclePhase::Ovulation => (
            "Confident & Magnetic",
            "Peak",
            "High",
            "Estrogen peaks with a testosterone surge. Peak physical and mental energy.",
        ),
        CyclePhase::EarlyLuteal => (
            "Calm & Nesting",
            "Moderate",
            "Baseline",
            "Progesterone rises — a calming, slower pace. Good for cozy routines.",
        ),
        CyclePhase::LateLuteal => (
            "Sensitive",
            "Low",
            "Low",
            "Hormones dropping — may feel irritable or foggy. Extra self-care helps.",
        ),
    };

    Some(PhaseInfo {
        phase,
        cycle_day,
        days_in_phase_remaining: days_remaining,
        mood,
        energy,
        libido,
        description,
    })
}

/// Returns a flat "stable baseline" PhaseInfo for users on hormonal birth control.
pub fn birth_control_phase(last_period_start: NaiveDate, today: NaiveDate, settings: &CycleSettings) -> Option<PhaseInfo> {
    let cycle_day = (today - last_period_start).num_days() + 1;
    if cycle_day < 1 || cycle_day > settings.average_cycle_length {
        return None;
    }
    Some(PhaseInfo {
        phase: CyclePhase::Follicular, // neutral default
        cycle_day,
        days_in_phase_remaining: settings.average_cycle_length - cycle_day,
        mood: "Stable",
        energy: "Steady",
        libido: "Steady",
        description: "Hormonal BC active — natural phase fluctuations are suppressed.",
    })
}

// --- Daily Mood Tracker ---

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MoodEntry {
    pub id: String,
    pub date: NaiveDate,
    pub mood: i32,      // 1-5
    pub energy: i32,    // 1-5
    pub libido: i32,    // 1-5
    pub notes: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PhaseInsight {
    pub phase: CyclePhase,
    pub sample_count: usize,
    pub avg_mood: f64,
    pub avg_energy: f64,
    pub avg_libido: f64,
    pub baseline_mood: f64,
    pub baseline_energy: f64,
    pub baseline_libido: f64,
    pub mood_deviation: f64,     // positive = user feels better than predicted
    pub energy_deviation: f64,
    pub libido_deviation: f64,
    pub insight_text: String,
}

/// Determine which phase a given date falls into, based on cycle history.
/// Finds the cycle whose start_date is <= date, then computes phase from that.
pub fn phase_for_date(
    date: NaiveDate,
    cycles: &[Cycle],
    settings: &CycleSettings,
) -> Option<CyclePhase> {
    // Cycles are sorted DESC by start_date. Find the one that contains this date.
    for cycle in cycles {
        if date >= cycle.start_date {
            let day = (date - cycle.start_date).num_days() + 1;
            if day >= 1 && day <= settings.average_cycle_length {
                return current_phase(cycle.start_date, date, settings).map(|p| p.phase);
            }
            // Past this cycle's expected length — date falls in a gap
            return None;
        }
    }
    None
}

/// Compute per-phase insights by comparing actual mood logs against textbook baselines.
/// Uses exponential decay weighting: recent cycles get more weight.
pub fn compute_insights(
    mood_logs: &[MoodEntry],
    cycles: &[Cycle],
    settings: &CycleSettings,
) -> Vec<PhaseInsight> {
    use std::collections::HashMap;

    if mood_logs.is_empty() || cycles.is_empty() {
        return Vec::new();
    }

    // For each mood log, determine which cycle it belongs to (for weighting)
    // and which phase it falls in.
    // cycles[0] is most recent = cycle_index 0 = weight 1.0
    // cycles[1] = cycle_index 1 = weight 0.8, etc.
    struct TaggedEntry {
        mood: f64,
        energy: f64,
        libido: f64,
        weight: f64,
    }

    let mut phase_entries: HashMap<CyclePhase, Vec<TaggedEntry>> = HashMap::new();

    for entry in mood_logs {
        // Find which cycle this entry belongs to
        let mut cycle_index = None;
        for (i, cycle) in cycles.iter().enumerate() {
            if entry.date >= cycle.start_date {
                let day = (entry.date - cycle.start_date).num_days() + 1;
                if day >= 1 && day <= settings.average_cycle_length {
                    cycle_index = Some(i);
                }
                break;
            }
        }

        let idx = match cycle_index {
            Some(i) => i,
            None => continue, // entry doesn't fall in any known cycle
        };

        let phase = match phase_for_date(entry.date, cycles, settings) {
            Some(p) => p,
            None => continue,
        };

        let weight = 0.8_f64.powi(idx as i32);

        phase_entries.entry(phase).or_default().push(TaggedEntry {
            mood: entry.mood as f64,
            energy: entry.energy as f64,
            libido: entry.libido as f64,
            weight,
        });
    }

    let mut insights = Vec::new();

    for phase in CyclePhase::all() {
        let entries = match phase_entries.get(phase) {
            Some(e) if !e.is_empty() => e,
            _ => continue,
        };

        let total_weight: f64 = entries.iter().map(|e| e.weight).sum();
        let avg_mood = entries.iter().map(|e| e.mood * e.weight).sum::<f64>() / total_weight;
        let avg_energy = entries.iter().map(|e| e.energy * e.weight).sum::<f64>() / total_weight;
        let avg_libido = entries.iter().map(|e| e.libido * e.weight).sum::<f64>() / total_weight;

        let (bm, be, bl) = phase.baseline_scores();
        let mood_dev = avg_mood - bm;
        let energy_dev = avg_energy - be;
        let libido_dev = avg_libido - bl;

        // Generate insight text
        let mut parts = Vec::new();
        let label = phase.label();

        if entries.len() < 3 {
            parts.push(format!("Only {} entries for {} — keep logging for better insights.", entries.len(), label));
        } else {
            // Find the most significant deviation
            let deviations = [
                ("mood", mood_dev),
                ("energy", energy_dev),
                ("drive", libido_dev),
            ];

            for (name, dev) in &deviations {
                if *dev > 0.8 {
                    parts.push(format!("Your {} is higher than typical during {}. (+{:.1})", name, label, dev));
                } else if *dev < -0.8 {
                    parts.push(format!("Your {} tends to dip more than expected during {}. ({:.1})", name, label, dev));
                }
            }

            if parts.is_empty() {
                parts.push(format!("Your {} phase matches the typical pattern.", label));
            }
        }

        insights.push(PhaseInsight {
            phase: phase.clone(),
            sample_count: entries.len(),
            avg_mood,
            avg_energy,
            avg_libido,
            baseline_mood: bm,
            baseline_energy: be,
            baseline_libido: bl,
            mood_deviation: mood_dev,
            energy_deviation: energy_dev,
            libido_deviation: libido_dev,
            insight_text: parts.join(" "),
        });
    }

    insights
}
