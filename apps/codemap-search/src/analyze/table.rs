//! Small, dependency-free tables. Keep potentially wide Unicode paths in the last column.

pub(super) fn print(title: &str, headings: &[(&str, bool)], rows: &[Vec<String>]) {
    let widths = headings
        .iter()
        .enumerate()
        .map(|(column, (heading, _))| {
            rows.iter()
                .map(|row| row[column].chars().count())
                .chain(std::iter::once(heading.len()))
                .max()
                .unwrap_or(0)
        })
        .collect::<Vec<_>>();
    let width = (widths.iter().sum::<usize>() + 2 * widths.len().saturating_sub(1)).clamp(72, 160);
    let border = "=".repeat(width);
    let separator = "-".repeat(width);
    println!("\n{title}\n{border}");
    let render = |row: &[String]| {
        row.iter()
            .enumerate()
            .map(|(i, value)| {
                if i + 1 == row.len() {
                    value.clone()
                } else if headings[i].1 {
                    format!("{:>width$}", value, width = widths[i])
                } else {
                    format!("{:width$}", value, width = widths[i])
                }
            })
            .collect::<Vec<_>>()
            .join("  ")
    };
    println!(
        "{}\n{separator}",
        render(
            &headings
                .iter()
                .map(|(label, _)| label.to_string())
                .collect::<Vec<_>>()
        )
    );
    for row in rows {
        println!("{}", render(row));
    }
    println!("{border}");
}

pub(super) fn size(size_bytes: Option<i64>) -> String {
    let Some(size_bytes) = size_bytes else {
        return "?".into();
    };
    let mut value = size_bytes as f64;
    let units = ["B", "KiB", "MiB", "GiB", "TiB", "PiB", "EiB"];
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < units.len() {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{size_bytes} B")
    } else {
        format!("{value:.2} {}", units[unit])
    }
}

pub(super) fn display_text(value: &str) -> String {
    value
        .chars()
        .flat_map(|ch| {
            if ch.is_control() {
                ch.escape_default().collect::<Vec<_>>()
            } else {
                vec![ch]
            }
        })
        .collect()
}
