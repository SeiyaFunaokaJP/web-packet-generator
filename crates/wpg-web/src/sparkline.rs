//! Self-drawn SVG sparkline (no chart library): pure path builder + component.

use std::collections::VecDeque;

use leptos::prelude::*;

/// Samples kept per series (spec §2.5 "直近の統計グラフ").
pub(crate) const SPARK_SAMPLES: usize = 60;

/// Appends `v`, dropping the oldest samples beyond `cap`.
pub(crate) fn push_sample(buf: &mut VecDeque<f64>, v: f64, cap: usize) {
    buf.push_back(if v.is_finite() { v.max(0.0) } else { 0.0 });
    while buf.len() > cap {
        buf.pop_front();
    }
}

/// Builds the SVG path of a line through `values` in a `width` × `height` box
/// (y grows downwards; the baseline is 0, the top is the maximum). Samples are
/// spread over the full width, right-aligned when fewer than `slots` are present
/// so the line scrolls in from the right. Returns `(line, area)`; both are empty
/// for no samples.
pub(crate) fn sparkline_paths(
    values: &[f64],
    slots: usize,
    width: f64,
    height: f64,
) -> (String, String) {
    if values.is_empty() || slots == 0 {
        return (String::new(), String::new());
    }
    let slots = slots.max(values.len()).max(2);
    let max = values.iter().copied().fold(0.0_f64, f64::max);
    let step = width / (slots - 1) as f64;
    let first_slot = slots - values.len();
    let pad = 1.0;
    let usable = (height - 2.0 * pad).max(0.0);
    let y = |v: f64| {
        if max > 0.0 {
            pad + usable * (1.0 - v / max)
        } else {
            height - pad
        }
    };
    let mut line = String::new();
    let points: Vec<(f64, f64)> = values
        .iter()
        .enumerate()
        .map(|(i, v)| ((first_slot + i) as f64 * step, y(*v)))
        .collect();
    // A single sample is drawn as a short flat segment ending at the right edge.
    let points = if points.len() == 1 {
        vec![(width - step, points[0].1), (width, points[0].1)]
    } else {
        points
    };
    for (i, (x, y)) in points.iter().enumerate() {
        let cmd = if i == 0 { 'M' } else { 'L' };
        line.push_str(&format!("{cmd}{} {} ", round2(*x), round2(*y)));
    }
    let line = line.trim_end().to_owned();
    let (x0, _) = points[0];
    let (xn, _) = points[points.len() - 1];
    let area = format!(
        "{line} L{} {} L{} {} Z",
        round2(xn),
        round2(height),
        round2(x0),
        round2(height)
    );
    (line, area)
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

/// A sparkline of `values` (newest last).
#[component]
pub(crate) fn Sparkline(
    /// Samples, oldest first.
    #[prop(into)]
    values: Signal<Vec<f64>>,
    /// Accessible label (e.g. "pps, last 30 s").
    #[prop(into)]
    label: Signal<String>,
    /// Extra classes (size / colour via `text-*`).
    #[prop(optional, into)]
    class: Option<String>,
) -> impl IntoView {
    let class = class.unwrap_or_default();
    const W: f64 = 120.0;
    const H: f64 = 32.0;
    let paths = Memo::new(move |_| values.with(|v| sparkline_paths(v, SPARK_SAMPLES, W, H)));
    view! {
        <svg
            class=format!("block {class}")
            viewBox=format!("0 0 {W} {H}")
            preserveAspectRatio="none"
            role="img"
            aria-label=move || label.get()
        >
            <path d=move || paths.get().1 fill="currentColor" fill-opacity="0.12" stroke="none"></path>
            <path
                d=move || paths.get().0
                fill="none"
                stroke="currentColor"
                stroke-width="1.5"
                stroke-linejoin="round"
                vector-effect="non-scaling-stroke"
            ></path>
        </svg>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_and_single() {
        assert_eq!(
            sparkline_paths(&[], 60, 100.0, 20.0),
            (String::new(), String::new())
        );
        let (line, area) = sparkline_paths(&[5.0], 11, 100.0, 20.0);
        assert_eq!(line, "M90 1 L100 1");
        assert!(area.ends_with("L100 20 L90 20 Z"), "{area}");
    }

    #[test]
    fn scales_to_max_and_right_aligns() {
        // 3 samples in 5 slots over width 100: x = 50, 75, 100.
        let (line, _) = sparkline_paths(&[0.0, 5.0, 10.0], 5, 100.0, 22.0);
        assert_eq!(line, "M50 21 L75 11 L100 1");
        // All zero: flat on the baseline.
        let (line, _) = sparkline_paths(&[0.0, 0.0], 2, 10.0, 10.0);
        assert_eq!(line, "M0 9 L10 9");
    }

    #[test]
    fn more_values_than_slots_use_all_values() {
        let (line, _) = sparkline_paths(&[1.0, 2.0, 3.0], 2, 10.0, 10.0);
        assert_eq!(line.matches('L').count(), 2);
    }

    #[test]
    fn push_caps_and_sanitises() {
        let mut b = VecDeque::new();
        for v in 0..70 {
            push_sample(&mut b, f64::from(v), SPARK_SAMPLES);
        }
        assert_eq!(b.len(), SPARK_SAMPLES);
        assert_eq!(b.front(), Some(&10.0));
        push_sample(&mut b, f64::NAN, SPARK_SAMPLES);
        push_sample(&mut b, -3.0, SPARK_SAMPLES);
        assert_eq!(b.back(), Some(&0.0));
    }
}
