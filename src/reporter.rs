use colored::*;
use groq_lint::rules::{Finding, Scope, Severity};
use terminal_size::{terminal_size, Width};

/// Get the terminal width if available, otherwise None.
fn get_terminal_width() -> Option<usize> {
    terminal_size().map(|(Width(w), _)| w as usize)
}

/// Colorize text within backticks and remove the backticks.
fn colorize_backticks(text: &str) -> String {
    let mut result = String::new();
    let mut in_backtick = false;
    let mut current_segment = String::new();

    for c in text.chars() {
        if c == '`' {
            if in_backtick {
                // End of backticked section - colorize it
                result.push_str(&current_segment.cyan().to_string());
                current_segment.clear();
            } else {
                // Start of backticked section - flush current
                result.push_str(&current_segment);
                current_segment.clear();
            }
            in_backtick = !in_backtick;
        } else {
            current_segment.push(c);
        }
    }

    // Flush remaining
    if in_backtick {
        // Unclosed backtick - just colorize what we have
        result.push_str(&current_segment.cyan().to_string());
    } else {
        result.push_str(&current_segment);
    }

    result
}

/// Strip ANSI escape codes from a string.
fn strip_ansi_codes(text: &str) -> String {
    let mut result = String::new();
    let mut chars = text.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // Skip until 'm' (end of ANSI sequence)
            while let Some(&next) = chars.peek() {
                chars.next();
                if next == 'm' {
                    break;
                }
            }
        } else {
            result.push(c);
        }
    }

    result
}

/// Wrap text to width, returning lines. Each line is colorized.
/// Returns a Vec of lines (without the prefix, which caller adds).
fn wrap_message(message: &str, width: usize) -> Vec<String> {
    let plain = strip_ansi_codes(message);
    let wrapped = textwrap::fill(&plain, width);
    wrapped.lines().map(colorize_backticks).collect()
}

/// Compute which lines should be visible based on findings and context.
/// If context_lines is 0, all lines are visible.
fn compute_visible_lines(
    findings_by_line: &[Vec<&Finding>],
    total_lines: usize,
    context_lines: usize,
) -> std::collections::HashSet<usize> {
    use std::collections::HashSet;

    let mut visible = HashSet::new();

    // If context is 0, show all lines
    if context_lines == 0 {
        for i in 0..total_lines {
            visible.insert(i);
        }
        return visible;
    }

    // Find lines with findings and add context around them
    for (i, findings) in findings_by_line.iter().enumerate() {
        if !findings.is_empty() {
            // Add the line with findings
            visible.insert(i);

            // Add context lines before
            let start = i.saturating_sub(context_lines);
            for j in start..i {
                visible.insert(j);
            }

            // Add context lines after
            let end = (i + context_lines + 1).min(total_lines);
            for j in (i + 1)..end {
                visible.insert(j);
            }
        }
    }

    visible
}

/// Build the prefix for continuation lines (wrapped message lines after the first).
/// This includes pipes for remaining findings plus spacing to align with the message.
fn build_continuation_prefix(
    query: &str,
    sorted_rev: &[&Finding],
    current_idx: usize,
    current_col: usize,
) -> String {
    let mut prefix = String::new();

    // Add pipes for findings that come after the current one
    for col in 0..current_col {
        let mut char_to_add = ' ';
        for (k, other) in sorted_rev.iter().enumerate() {
            if k > current_idx {
                let other_col = find_line_col(query, other.span.start).1;
                if other_col == col {
                    char_to_add = '│';
                }
            }
        }
        if char_to_add == '│' {
            prefix.push_str(&"│".red().to_string());
        } else {
            prefix.push(char_to_add);
        }
    }

    // Add spacing to align with the message text (after "└── ")
    prefix.push_str("    "); // 4 spaces to match "└── "

    prefix
}

pub fn print_report(query: &str, findings: &[Finding], context_lines: usize) {
    if findings.is_empty() {
        println!("{}", "✅ No issues found.".green());
        return;
    }

    // Count findings by severity
    let mut high_count = 0;
    let mut medium_count = 0;
    let mut low_count = 0;
    for f in findings {
        match f.severity {
            Severity::High => high_count += 1,
            Severity::Medium => medium_count += 1,
            Severity::Low => low_count += 1,
        }
    }

    // Build severity breakdown
    let mut parts = Vec::new();
    if high_count > 0 {
        parts.push(format!("{} high", high_count));
    }
    if medium_count > 0 {
        parts.push(format!("{} medium", medium_count));
    }
    if low_count > 0 {
        parts.push(format!("{} low", low_count));
    }

    // Print summary heading
    let total = findings.len();
    let issue_word = if total == 1 { "issue" } else { "issues" };
    let heading = if parts.is_empty() {
        format!("⚠ {} {} found:", total, issue_word)
    } else {
        format!("⚠ {} {} found ({}):", total, issue_word, parts.join(", "))
    };
    println!("{}", heading.yellow().bold());
    println!();

    // Separate global and node findings
    let mut node_findings = vec![];
    let mut global_findings = vec![];

    for f in findings {
        match f.scope {
            Scope::Node => node_findings.push(f),
            Scope::Global => global_findings.push(f),
        }
    }

    let lines: Vec<&str> = query.lines().collect();
    let mut findings_by_line: Vec<Vec<&Finding>> = vec![vec![]; lines.len()];

    for finding in &node_findings {
        let start = find_line_col(query, finding.span.start);
        if start.0 < lines.len() {
            findings_by_line[start.0].push(finding);
        }
    }

    // Determine which lines to show based on context
    let visible_lines = compute_visible_lines(&findings_by_line, lines.len(), context_lines);

    // Calculate line number width for formatting
    let line_num_width = lines.len().to_string().len();
    let gutter_width = line_num_width + 3; // " │ " after number

    let mut last_printed_line: Option<usize> = None;

    for (i, line) in lines.iter().enumerate() {
        // Skip lines not in the visible set
        if !visible_lines.contains(&i) {
            continue;
        }

        // Print separator if there's a gap
        if let Some(last) = last_printed_line {
            if i > last + 1 {
                let skipped = i - last - 1;
                let gap_indicator = format!("{:>width$} │", "⋮", width = line_num_width);
                if skipped == 1 {
                    println!("{}", gap_indicator.dimmed());
                } else {
                    println!(
                        "{} {}",
                        gap_indicator.dimmed(),
                        format!("({} lines)", skipped).dimmed()
                    );
                }
            }
        }
        last_printed_line = Some(i);

        // Format line number (1-based)
        let line_num = format!("{:>width$} │ ", i + 1, width = line_num_width);
        let gutter_indent = " ".repeat(gutter_width);

        let line_findings = &findings_by_line[i];
        if line_findings.is_empty() {
            println!("{}{}", line_num.dimmed(), line);
            continue;
        }

        // 1. Print highlighted line with line number
        let mut mask = vec![false; line.len()];
        for f in line_findings {
            let start = find_line_col(query, f.span.start).1;
            let end = find_line_col(query, f.span.end).1;
            for k in start..end.min(mask.len()) {
                mask[k] = true;
            }
        }

        let mut highlighted = String::new();
        let mut in_hl = false;
        for (k, c) in line.chars().enumerate() {
            if k < mask.len() && mask[k] && !in_hl {
                highlighted.push_str("\x1b[41m");
                in_hl = true;
            } else if (k >= mask.len() || !mask[k]) && in_hl {
                highlighted.push_str("\x1b[0m");
                in_hl = false;
            }
            highlighted.push(c);
        }
        if in_hl {
            highlighted.push_str("\x1b[0m");
        }
        println!("{}{}", line_num.dimmed(), highlighted);

        let mut sorted_findings = line_findings.to_vec();
        sorted_findings.sort_by_key(|f| f.span.start);

        // Draw ASCII Art (indented by gutter width)
        // 1. Tildes
        let mut tildes = vec![' '; line.len().max(1)];
        for f in line_findings {
            let start = find_line_col(query, f.span.start).1;
            let end = find_line_col(query, f.span.end).1;
            for k in start..end.min(tildes.len()) {
                tildes[k] = '~';
            }
        }
        println!("{}{}", gutter_indent, String::from_iter(tildes).red());

        // 2. Arrows
        let mut arrows = vec![' '; line.len().max(1)];
        for f in line_findings {
            let start = find_line_col(query, f.span.start).1;
            if start < arrows.len() {
                arrows[start] = '▲';
            }
        }
        println!("{}{}", gutter_indent, String::from_iter(arrows).red());

        // 3. Initial vertical line (│)
        let mut pipe_line = vec![' '; line.len().max(1)];
        for f in line_findings {
            let start = find_line_col(query, f.span.start).1;
            if start < pipe_line.len() {
                pipe_line[start] = '│';
            }
        }
        println!("{}{}", gutter_indent, String::from_iter(pipe_line).red());

        // Messages from right to left
        let mut sorted_rev = sorted_findings.clone();
        sorted_rev.reverse();

        for (idx, f) in sorted_rev.iter().enumerate() {
            // Build the prefix for this message (pipes for remaining findings)
            let mut prefix = String::new();
            let mut prefix_plain = String::new(); // For width calculation
            let current_col = find_line_col(query, f.span.start).1;

            for col in 0..current_col {
                let mut char_to_add = ' ';
                for (k, other) in sorted_rev.iter().enumerate() {
                    if k > idx {
                        let other_col = find_line_col(query, other.span.start).1;
                        if other_col == col {
                            char_to_add = '│';
                        }
                    }
                }
                if char_to_add == '│' {
                    prefix.push_str(&"│".red().to_string());
                } else {
                    prefix.push(char_to_add);
                }
                prefix_plain.push(char_to_add);
            }

            // Add the └── connector
            let connector = "└── ".red().to_string();
            let connector_len = 4; // "└── " is 4 chars

            // Calculate available width for message
            let msg_start_col = gutter_width + current_col + connector_len;
            let available_width = get_terminal_width()
                .map(|w| w.saturating_sub(msg_start_col))
                .filter(|&w| w > 20);

            // Wrap and colorize the message
            let message_lines = if let Some(width) = available_width {
                wrap_message(&f.message, width)
            } else {
                vec![colorize_backticks(&f.message)]
            };

            // Print first line with └── connector
            println!(
                "{}{}{}{}",
                gutter_indent, prefix, connector, message_lines[0]
            );

            // Print continuation lines with proper prefix (pipes aligned)
            let continuation_prefix =
                build_continuation_prefix(query, &sorted_rev, idx, current_col);
            for cont_line in message_lines.iter().skip(1) {
                println!("{}{}{}", gutter_indent, continuation_prefix, cont_line);
            }

            // Add blank spacing row if not the last one
            if idx < sorted_rev.len() - 1 {
                let mut space_line = String::new();
                for col in 0..=current_col {
                    let mut char_to_add = ' ';
                    for (k, other) in sorted_rev.iter().enumerate() {
                        if k > idx {
                            let other_col = find_line_col(query, other.span.start).1;
                            if other_col == col {
                                char_to_add = '│';
                            }
                        }
                    }
                    if char_to_add == '│' {
                        space_line.push_str(&"│".red().to_string());
                    } else {
                        space_line.push(char_to_add);
                    }
                }
                println!("{}{}", gutter_indent, space_line);
            }
        }

        println!();
    }

    // Print global findings at the end
    if !global_findings.is_empty() {
        println!();
        println!("{}", "Global Issues:".bold().yellow());
        for f in global_findings {
            let indent = 2; // "• " prefix
            let available_width = get_terminal_width()
                .map(|w| w.saturating_sub(indent))
                .filter(|&w| w > 20);

            let message_lines = if let Some(width) = available_width {
                wrap_message(&f.message, width)
            } else {
                vec![colorize_backticks(&f.message)]
            };

            // Print first line with bullet
            println!("{} {}", "•".yellow(), message_lines[0]);

            // Print continuation lines with indentation
            for cont_line in message_lines.iter().skip(1) {
                println!("  {}", cont_line);
            }
        }
        println!();
    }
}

fn find_line_col(query: &str, offset: usize) -> (usize, usize) {
    let mut line = 0;
    let mut col = 0;
    for (i, char) in query.char_indices() {
        if i == offset {
            break;
        }
        if char == '\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
    }
    (line, col)
}
