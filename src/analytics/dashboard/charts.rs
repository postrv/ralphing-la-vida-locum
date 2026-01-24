//! SVG chart generation for analytics dashboard.
//!
//! This module provides simple SVG chart generators for visualizing
//! analytics data. All charts are generated as inline SVG elements
//! with no external dependencies.

use chrono::{DateTime, Utc};

/// Data point for chart visualization.
#[derive(Debug, Clone)]
pub struct ChartDataPoint {
    /// Label for this data point (e.g., date, category name).
    pub label: String,
    /// Numeric value for this data point.
    pub value: f64,
}

impl ChartDataPoint {
    /// Create a new chart data point.
    ///
    /// # Arguments
    ///
    /// * `label` - Display label for this point
    /// * `value` - Numeric value
    #[must_use]
    pub fn new(label: impl Into<String>, value: f64) -> Self {
        Self {
            label: label.into(),
            value,
        }
    }

    /// Create a data point from a timestamp and value.
    ///
    /// Formats the timestamp as a short date string.
    #[must_use]
    pub fn from_timestamp(timestamp: DateTime<Utc>, value: f64) -> Self {
        Self {
            label: timestamp.format("%m/%d").to_string(),
            value,
        }
    }
}

/// Configuration for chart rendering.
#[derive(Debug, Clone)]
pub struct ChartConfig {
    /// Width of the chart in pixels.
    pub width: u32,
    /// Height of the chart in pixels.
    pub height: u32,
    /// Primary color for chart elements (hex without #).
    pub primary_color: String,
    /// Secondary color for chart elements (hex without #).
    pub secondary_color: String,
    /// Background color (hex without #).
    pub background_color: String,
    /// Text color (hex without #).
    pub text_color: String,
    /// Whether to show grid lines.
    pub show_grid: bool,
    /// Padding around the chart content.
    pub padding: u32,
}

impl Default for ChartConfig {
    fn default() -> Self {
        Self {
            width: 400,
            height: 200,
            primary_color: "3b82f6".to_string(),
            secondary_color: "22c55e".to_string(),
            background_color: "1e293b".to_string(),
            text_color: "94a3b8".to_string(),
            show_grid: true,
            padding: 40,
        }
    }
}

/// Line chart generator for time-series data.
///
/// Generates an SVG line chart suitable for displaying trends over time.
///
/// # Example
///
/// ```
/// use ralph::analytics::dashboard::charts::{LineChart, ChartDataPoint, ChartConfig};
///
/// let data = vec![
///     ChartDataPoint::new("Jan", 10.0),
///     ChartDataPoint::new("Feb", 15.0),
///     ChartDataPoint::new("Mar", 12.0),
/// ];
/// let chart = LineChart::new(data, ChartConfig::default());
/// let svg = chart.render();
/// assert!(svg.contains("<svg"));
/// ```
#[derive(Debug)]
pub struct LineChart {
    data: Vec<ChartDataPoint>,
    config: ChartConfig,
}

impl LineChart {
    /// Create a new line chart with the given data and configuration.
    #[must_use]
    pub fn new(data: Vec<ChartDataPoint>, config: ChartConfig) -> Self {
        Self { data, config }
    }

    /// Render the chart as an SVG string.
    #[must_use]
    pub fn render(&self) -> String {
        if self.data.is_empty() {
            return self.render_empty_state();
        }

        let ChartConfig {
            width,
            height,
            ref primary_color,
            ref background_color,
            show_grid,
            padding,
            ..
        } = self.config;

        // Pre-compute color values with # prefix
        let bg_color = format!("#{background_color}");
        let stroke_color = format!("#{primary_color}");

        let chart_width = width - 2 * padding;
        let chart_height = height - 2 * padding;

        // Find min/max values for scaling
        let max_value = self
            .data
            .iter()
            .map(|p| p.value)
            .fold(f64::NEG_INFINITY, f64::max);
        let min_value = self
            .data
            .iter()
            .map(|p| p.value)
            .fold(f64::INFINITY, f64::min);

        // Ensure we have a range (avoid division by zero)
        let value_range = if (max_value - min_value).abs() < f64::EPSILON {
            1.0
        } else {
            max_value - min_value
        };

        // Generate path points
        let points: Vec<String> = self
            .data
            .iter()
            .enumerate()
            .map(|(i, point)| {
                let x = if self.data.len() > 1 {
                    padding as f64 + (i as f64 / (self.data.len() - 1) as f64) * chart_width as f64
                } else {
                    padding as f64 + chart_width as f64 / 2.0
                };
                let y = padding as f64
                    + chart_height as f64 * (1.0 - (point.value - min_value) / value_range);
                format!("{:.1},{:.1}", x, y)
            })
            .collect();

        let path_data = if points.len() == 1 {
            // Single point - draw a small circle marker instead of a line
            String::new()
        } else {
            format!("M {} L {}", points[0], points[1..].join(" L "))
        };

        // Generate grid lines
        let grid_lines = if show_grid {
            self.render_grid(padding, chart_width, chart_height)
        } else {
            String::new()
        };

        // Generate data point markers
        let markers = self.render_markers(&points, primary_color);

        // Generate x-axis labels (show first, middle, last)
        let labels = self.render_x_labels(padding, chart_width, chart_height);

        format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {width} {height}" class="chart line-chart">
  <rect width="{width}" height="{height}" fill="{bg_color}"/>
  {grid_lines}
  <path d="{path_data}" fill="none" stroke="{stroke_color}" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>
  {markers}
  {labels}
</svg>"##,
        )
    }

    fn render_empty_state(&self) -> String {
        let ChartConfig {
            width,
            height,
            ref background_color,
            ref text_color,
            ..
        } = self.config;

        let bg_color = format!("#{background_color}");
        let txt_color = format!("#{text_color}");
        let cx = width / 2;
        let cy = height / 2;

        format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {width} {height}" class="chart line-chart">
  <rect width="{width}" height="{height}" fill="{bg_color}"/>
  <text x="{cx}" y="{cy}" text-anchor="middle" fill="{txt_color}" font-size="14">No data available</text>
</svg>"##,
        )
    }

    fn render_grid(&self, padding: u32, chart_width: u32, chart_height: u32) -> String {
        let mut lines = String::new();
        let grid_color = "#334155";

        // Horizontal grid lines (5 lines)
        for i in 0..=4 {
            let y = padding + (chart_height * i / 4);
            lines.push_str(&format!(
                r#"  <line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-opacity="0.5"/>"#,
                padding,
                y,
                padding + chart_width,
                y,
                grid_color
            ));
            lines.push('\n');
        }

        lines
    }

    fn render_markers(&self, points: &[String], color: &str) -> String {
        let fill_color = format!("#{}", color);
        points
            .iter()
            .map(|point| {
                let coords: Vec<&str> = point.split(',').collect();
                format!(
                    r#"  <circle cx="{}" cy="{}" r="4" fill="{}" />"#,
                    coords[0], coords[1], fill_color
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn render_x_labels(&self, padding: u32, chart_width: u32, chart_height: u32) -> String {
        if self.data.is_empty() {
            return String::new();
        }

        let text_color = format!("#{}", self.config.text_color);
        let y = padding + chart_height + 15;
        let mut labels = Vec::new();

        // First label
        labels.push(format!(
            r#"  <text x="{}" y="{}" text-anchor="start" fill="{}" font-size="10">{}</text>"#,
            padding,
            y,
            text_color,
            escape_xml(&self.data[0].label)
        ));

        // Last label (if more than 1 point)
        if self.data.len() > 1 {
            labels.push(format!(
                r#"  <text x="{}" y="{}" text-anchor="end" fill="{}" font-size="10">{}</text>"#,
                padding + chart_width,
                y,
                text_color,
                escape_xml(&self.data[self.data.len() - 1].label)
            ));
        }

        labels.join("\n")
    }
}

/// Bar chart generator for categorical data.
///
/// Generates an SVG bar chart suitable for displaying pass/fail or category counts.
///
/// # Example
///
/// ```
/// use ralph::analytics::dashboard::charts::{BarChart, ChartDataPoint, ChartConfig};
///
/// let data = vec![
///     ChartDataPoint::new("Passed", 95.0),
///     ChartDataPoint::new("Failed", 5.0),
/// ];
/// let chart = BarChart::new(data, ChartConfig::default());
/// let svg = chart.render();
/// assert!(svg.contains("<svg"));
/// ```
#[derive(Debug)]
pub struct BarChart {
    data: Vec<ChartDataPoint>,
    config: ChartConfig,
}

impl BarChart {
    /// Create a new bar chart with the given data and configuration.
    #[must_use]
    pub fn new(data: Vec<ChartDataPoint>, config: ChartConfig) -> Self {
        Self { data, config }
    }

    /// Render the chart as an SVG string.
    #[must_use]
    pub fn render(&self) -> String {
        if self.data.is_empty() {
            return self.render_empty_state();
        }

        let ChartConfig {
            width,
            height,
            ref primary_color,
            ref secondary_color,
            ref background_color,
            ref text_color,
            padding,
            ..
        } = self.config;

        let chart_width = width - 2 * padding;
        let chart_height = height - 2 * padding;

        // Find max value for scaling
        let max_value = self
            .data
            .iter()
            .map(|p| p.value)
            .fold(0.0_f64, f64::max)
            .max(1.0); // Ensure minimum of 1 to avoid division by zero

        // Calculate bar dimensions
        let bar_count = self.data.len();
        let gap = 10;
        let bar_width = (chart_width as usize - gap * (bar_count + 1)) / bar_count;

        // Alternate colors for bars
        let colors = [
            format!("#{}", primary_color),
            format!("#{}", secondary_color),
        ];

        let bars: Vec<String> = self
            .data
            .iter()
            .enumerate()
            .map(|(i, point)| {
                let x = padding as usize + gap + i * (bar_width + gap);
                let bar_height = ((point.value / max_value) * chart_height as f64).round() as u32;
                let y = padding + chart_height - bar_height;
                let color = &colors[i % colors.len()];

                format!(
                    r#"  <rect x="{}" y="{}" width="{}" height="{}" fill="{}" rx="4"/>"#,
                    x, y, bar_width, bar_height, color
                )
            })
            .collect();

        let txt_color = format!("#{}", text_color);
        let labels: Vec<String> = self
            .data
            .iter()
            .enumerate()
            .map(|(i, point)| {
                let x = padding as usize + gap + i * (bar_width + gap) + bar_width / 2;
                let y = padding + chart_height + 15;
                format!(
                    r#"  <text x="{}" y="{}" text-anchor="middle" fill="{}" font-size="10">{}</text>"#,
                    x, y, txt_color, escape_xml(&point.label)
                )
            })
            .collect();

        // Value labels on top of bars
        let value_labels: Vec<String> = self
            .data
            .iter()
            .enumerate()
            .map(|(i, point)| {
                let x = padding as usize + gap + i * (bar_width + gap) + bar_width / 2;
                let bar_height =
                    ((point.value / max_value) * chart_height as f64).round() as u32;
                let y = padding + chart_height - bar_height - 5;
                format!(
                    r#"  <text x="{}" y="{}" text-anchor="middle" fill="{}" font-size="11" font-weight="bold">{}</text>"#,
                    x, y, txt_color, point.value.round() as i64
                )
            })
            .collect();

        let bg_color = format!("#{background_color}");
        let bars = bars.join("\n");
        let labels = labels.join("\n");
        let value_labels = value_labels.join("\n");

        format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {width} {height}" class="chart bar-chart">
  <rect width="{width}" height="{height}" fill="{bg_color}"/>
{bars}
{labels}
{value_labels}
</svg>"##,
        )
    }

    fn render_empty_state(&self) -> String {
        let ChartConfig {
            width,
            height,
            ref background_color,
            ref text_color,
            ..
        } = self.config;

        let bg_color = format!("#{background_color}");
        let txt_color = format!("#{text_color}");
        let cx = width / 2;
        let cy = height / 2;

        format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {width} {height}" class="chart bar-chart">
  <rect width="{width}" height="{height}" fill="{bg_color}"/>
  <text x="{cx}" y="{cy}" text-anchor="middle" fill="{txt_color}" font-size="14">No data available</text>
</svg>"##,
        )
    }
}

/// Pie chart generator for proportional data.
///
/// Generates an SVG pie chart suitable for displaying time distribution or proportions.
///
/// # Example
///
/// ```
/// use ralph::analytics::dashboard::charts::{PieChart, ChartDataPoint, ChartConfig};
///
/// let data = vec![
///     ChartDataPoint::new("Build", 45.0),
///     ChartDataPoint::new("Test", 30.0),
///     ChartDataPoint::new("Deploy", 25.0),
/// ];
/// let chart = PieChart::new(data, ChartConfig::default());
/// let svg = chart.render();
/// assert!(svg.contains("<svg"));
/// ```
#[derive(Debug)]
pub struct PieChart {
    data: Vec<ChartDataPoint>,
    config: ChartConfig,
}

impl PieChart {
    /// Create a new pie chart with the given data and configuration.
    #[must_use]
    pub fn new(data: Vec<ChartDataPoint>, config: ChartConfig) -> Self {
        Self { data, config }
    }

    /// Render the chart as an SVG string.
    #[must_use]
    pub fn render(&self) -> String {
        if self.data.is_empty() {
            return self.render_empty_state();
        }

        let ChartConfig {
            width,
            height,
            ref background_color,
            ref text_color,
            ..
        } = self.config;

        let cx = width as f64 / 2.0;
        let cy = height as f64 / 2.0;
        let radius = (width.min(height) as f64 / 2.0) - 40.0;

        let total: f64 = self.data.iter().map(|p| p.value).sum();
        if total <= 0.0 {
            return self.render_empty_state();
        }

        // Color palette for slices
        let colors = [
            "#3b82f6", "#22c55e", "#f59e0b", "#ef4444", "#8b5cf6", "#06b6d4",
        ];

        let mut slices = Vec::new();
        let mut legends = Vec::new();
        let mut start_angle = -90.0_f64; // Start from top

        let txt_color = format!("#{}", text_color);
        let bg_color = format!("#{}", background_color);

        for (i, point) in self.data.iter().enumerate() {
            let slice_angle = (point.value / total) * 360.0;
            let end_angle = start_angle + slice_angle;
            let color = colors[i % colors.len()];

            // Generate arc path
            let path = self.arc_path(cx, cy, radius, start_angle, end_angle);
            slices.push(format!(
                r#"  <path d="{}" fill="{}" stroke="{}" stroke-width="1"/>"#,
                path, color, bg_color
            ));

            // Legend entry
            let legend_y = 20.0 + (i as f64 * 20.0);
            let percentage = (point.value / total * 100.0).round() as i32;
            legends.push(format!(
                r#"  <rect x="10" y="{}" width="12" height="12" fill="{}"/>
  <text x="28" y="{}" fill="{}" font-size="11">{} ({}%)</text>"#,
                legend_y - 10.0,
                color,
                legend_y,
                txt_color,
                escape_xml(&point.label),
                percentage
            ));

            start_angle = end_angle;
        }

        format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {width} {height}" class="chart pie-chart">
  <rect width="{width}" height="{height}" fill="{bg_color}"/>
{slices}
{legends}
</svg>"##,
            width = width,
            height = height,
            bg_color = bg_color,
            slices = slices.join("\n"),
            legends = legends.join("\n"),
        )
    }

    fn arc_path(&self, cx: f64, cy: f64, radius: f64, start_angle: f64, end_angle: f64) -> String {
        let start_rad = start_angle.to_radians();
        let end_rad = end_angle.to_radians();

        let x1 = cx + radius * start_rad.cos();
        let y1 = cy + radius * start_rad.sin();
        let x2 = cx + radius * end_rad.cos();
        let y2 = cy + radius * end_rad.sin();

        let large_arc = if (end_angle - start_angle).abs() > 180.0 {
            1
        } else {
            0
        };

        format!(
            "M {:.1} {:.1} L {:.1} {:.1} A {:.1} {:.1} 0 {} 1 {:.1} {:.1} Z",
            cx, cy, x1, y1, radius, radius, large_arc, x2, y2
        )
    }

    fn render_empty_state(&self) -> String {
        let ChartConfig {
            width,
            height,
            ref background_color,
            ref text_color,
            ..
        } = self.config;

        let bg_color = format!("#{background_color}");
        let txt_color = format!("#{text_color}");
        let cx = width / 2;
        let cy = height / 2;

        format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {width} {height}" class="chart pie-chart">
  <rect width="{width}" height="{height}" fill="{bg_color}"/>
  <text x="{cx}" y="{cy}" text-anchor="middle" fill="{txt_color}" font-size="14">No data available</text>
</svg>"##,
        )
    }
}

/// Escape special XML/SVG characters.
fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // Phase 25.3: Chart Generation Tests
    // =========================================================================

    // -------------------------------------------------------------------------
    // Line Chart Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_line_chart_generates_valid_svg() {
        // Given: Sample trend data
        let data = vec![
            ChartDataPoint::new("Day 1", 10.0),
            ChartDataPoint::new("Day 2", 15.0),
            ChartDataPoint::new("Day 3", 12.0),
            ChartDataPoint::new("Day 4", 18.0),
            ChartDataPoint::new("Day 5", 14.0),
        ];
        let chart = LineChart::new(data, ChartConfig::default());

        // When: Rendering the chart
        let svg = chart.render();

        // Then: Output should be valid SVG
        assert!(svg.contains("<svg"), "Must contain SVG opening tag");
        assert!(svg.contains("</svg>"), "Must contain SVG closing tag");
        assert!(
            svg.contains("xmlns=\"http://www.w3.org/2000/svg\""),
            "Must have SVG namespace"
        );
        assert!(svg.contains("viewBox"), "Must have viewBox attribute");
        assert!(
            svg.contains("<path"),
            "Line chart must contain path element"
        );
        assert!(svg.contains("line-chart"), "Must have line-chart class");
    }

    #[test]
    fn test_line_chart_has_data_points_markers() {
        // Given: Data with specific points
        let data = vec![
            ChartDataPoint::new("A", 5.0),
            ChartDataPoint::new("B", 10.0),
            ChartDataPoint::new("C", 7.0),
        ];
        let chart = LineChart::new(data, ChartConfig::default());

        // When: Rendering
        let svg = chart.render();

        // Then: Should have circle markers for each point
        let circle_count = svg.matches("<circle").count();
        assert_eq!(
            circle_count, 3,
            "Should have 3 circle markers for 3 data points"
        );
    }

    #[test]
    fn test_line_chart_with_single_point() {
        // Given: Single data point
        let data = vec![ChartDataPoint::new("Only", 42.0)];
        let chart = LineChart::new(data, ChartConfig::default());

        // When: Rendering
        let svg = chart.render();

        // Then: Should still produce valid SVG
        assert!(svg.contains("<svg"), "Must produce valid SVG");
        assert!(svg.contains("<circle"), "Single point should have a marker");
    }

    #[test]
    fn test_line_chart_handles_empty_data() {
        // Given: Empty data
        let data: Vec<ChartDataPoint> = vec![];
        let chart = LineChart::new(data, ChartConfig::default());

        // When: Rendering
        let svg = chart.render();

        // Then: Should produce valid SVG with "no data" message
        assert!(svg.contains("<svg"), "Must produce valid SVG");
        assert!(svg.contains("</svg>"), "Must close SVG tag");
        assert!(
            svg.contains("No data") || svg.contains("no data"),
            "Empty chart should show no data message"
        );
    }

    #[test]
    fn test_line_chart_handles_identical_values() {
        // Given: All identical values
        let data = vec![
            ChartDataPoint::new("A", 5.0),
            ChartDataPoint::new("B", 5.0),
            ChartDataPoint::new("C", 5.0),
        ];
        let chart = LineChart::new(data, ChartConfig::default());

        // When: Rendering
        let svg = chart.render();

        // Then: Should not panic and produce valid SVG
        assert!(svg.contains("<svg"), "Must produce valid SVG");
        assert!(!svg.contains("NaN"), "Must not contain NaN values");
        assert!(
            !svg.contains("Infinity"),
            "Must not contain Infinity values"
        );
    }

    #[test]
    fn test_line_chart_respects_config() {
        // Given: Custom config
        let config = ChartConfig {
            width: 600,
            height: 300,
            primary_color: "ff0000".to_string(),
            ..ChartConfig::default()
        };
        let data = vec![ChartDataPoint::new("A", 1.0), ChartDataPoint::new("B", 2.0)];
        let chart = LineChart::new(data, config);

        // When: Rendering
        let svg = chart.render();

        // Then: Should use custom dimensions and color
        assert!(svg.contains("600"), "Should use custom width");
        assert!(svg.contains("300"), "Should use custom height");
        assert!(svg.contains("ff0000"), "Should use custom primary color");
    }

    // -------------------------------------------------------------------------
    // Bar Chart Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_bar_chart_generates_valid_svg() {
        // Given: Sample category data
        let data = vec![
            ChartDataPoint::new("Passed", 95.0),
            ChartDataPoint::new("Failed", 5.0),
        ];
        let chart = BarChart::new(data, ChartConfig::default());

        // When: Rendering the chart
        let svg = chart.render();

        // Then: Output should be valid SVG
        assert!(svg.contains("<svg"), "Must contain SVG opening tag");
        assert!(svg.contains("</svg>"), "Must contain SVG closing tag");
        assert!(
            svg.contains("xmlns=\"http://www.w3.org/2000/svg\""),
            "Must have SVG namespace"
        );
        assert!(
            svg.contains("<rect"),
            "Bar chart must contain rect elements"
        );
        assert!(svg.contains("bar-chart"), "Must have bar-chart class");
    }

    #[test]
    fn test_bar_chart_has_bars_for_each_category() {
        // Given: Data with 3 categories
        let data = vec![
            ChartDataPoint::new("A", 10.0),
            ChartDataPoint::new("B", 20.0),
            ChartDataPoint::new("C", 15.0),
        ];
        let chart = BarChart::new(data, ChartConfig::default());

        // When: Rendering
        let svg = chart.render();

        // Then: Should have rect elements (1 background + 3 bars)
        let rect_count = svg.matches("<rect").count();
        assert!(
            rect_count >= 4,
            "Should have at least 4 rects (1 bg + 3 bars)"
        );
    }

    #[test]
    fn test_bar_chart_handles_empty_data() {
        // Given: Empty data
        let data: Vec<ChartDataPoint> = vec![];
        let chart = BarChart::new(data, ChartConfig::default());

        // When: Rendering
        let svg = chart.render();

        // Then: Should produce valid SVG with "no data" message
        assert!(svg.contains("<svg"), "Must produce valid SVG");
        assert!(
            svg.contains("No data") || svg.contains("no data"),
            "Empty chart should show no data message"
        );
    }

    #[test]
    fn test_bar_chart_shows_labels() {
        // Given: Data with labels
        let data = vec![
            ChartDataPoint::new("Passed", 95.0),
            ChartDataPoint::new("Failed", 5.0),
        ];
        let chart = BarChart::new(data, ChartConfig::default());

        // When: Rendering
        let svg = chart.render();

        // Then: Labels should appear in SVG
        assert!(svg.contains("Passed"), "Should contain 'Passed' label");
        assert!(svg.contains("Failed"), "Should contain 'Failed' label");
    }

    #[test]
    fn test_bar_chart_shows_values() {
        // Given: Data with specific values
        let data = vec![
            ChartDataPoint::new("A", 42.0),
            ChartDataPoint::new("B", 17.0),
        ];
        let chart = BarChart::new(data, ChartConfig::default());

        // When: Rendering
        let svg = chart.render();

        // Then: Values should appear in SVG
        assert!(svg.contains("42"), "Should show value 42");
        assert!(svg.contains("17"), "Should show value 17");
    }

    #[test]
    fn test_bar_chart_handles_zero_values() {
        // Given: Data with zero value
        let data = vec![
            ChartDataPoint::new("Has Data", 100.0),
            ChartDataPoint::new("No Data", 0.0),
        ];
        let chart = BarChart::new(data, ChartConfig::default());

        // When: Rendering
        let svg = chart.render();

        // Then: Should produce valid SVG without errors
        assert!(svg.contains("<svg"), "Must produce valid SVG");
        assert!(!svg.contains("NaN"), "Must not contain NaN values");
    }

    // -------------------------------------------------------------------------
    // Pie Chart Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_pie_chart_generates_valid_svg() {
        // Given: Sample proportional data
        let data = vec![
            ChartDataPoint::new("Build", 45.0),
            ChartDataPoint::new("Test", 30.0),
            ChartDataPoint::new("Deploy", 25.0),
        ];
        let chart = PieChart::new(data, ChartConfig::default());

        // When: Rendering the chart
        let svg = chart.render();

        // Then: Output should be valid SVG
        assert!(svg.contains("<svg"), "Must contain SVG opening tag");
        assert!(svg.contains("</svg>"), "Must contain SVG closing tag");
        assert!(
            svg.contains("xmlns=\"http://www.w3.org/2000/svg\""),
            "Must have SVG namespace"
        );
        assert!(
            svg.contains("<path"),
            "Pie chart must contain path elements for slices"
        );
        assert!(svg.contains("pie-chart"), "Must have pie-chart class");
    }

    #[test]
    fn test_pie_chart_has_slices_for_each_segment() {
        // Given: Data with 4 segments
        let data = vec![
            ChartDataPoint::new("A", 25.0),
            ChartDataPoint::new("B", 25.0),
            ChartDataPoint::new("C", 25.0),
            ChartDataPoint::new("D", 25.0),
        ];
        let chart = PieChart::new(data, ChartConfig::default());

        // When: Rendering
        let svg = chart.render();

        // Then: Should have path elements for each slice
        let path_count = svg.matches("<path").count();
        assert_eq!(path_count, 4, "Should have 4 path elements for 4 slices");
    }

    #[test]
    fn test_pie_chart_handles_empty_data() {
        // Given: Empty data
        let data: Vec<ChartDataPoint> = vec![];
        let chart = PieChart::new(data, ChartConfig::default());

        // When: Rendering
        let svg = chart.render();

        // Then: Should produce valid SVG with "no data" message
        assert!(svg.contains("<svg"), "Must produce valid SVG");
        assert!(
            svg.contains("No data") || svg.contains("no data"),
            "Empty chart should show no data message"
        );
    }

    #[test]
    fn test_pie_chart_shows_legend() {
        // Given: Data with labels
        let data = vec![
            ChartDataPoint::new("Build", 60.0),
            ChartDataPoint::new("Test", 40.0),
        ];
        let chart = PieChart::new(data, ChartConfig::default());

        // When: Rendering
        let svg = chart.render();

        // Then: Legend should show labels and percentages
        assert!(svg.contains("Build"), "Should contain 'Build' label");
        assert!(svg.contains("Test"), "Should contain 'Test' label");
        assert!(svg.contains("%"), "Should show percentages");
    }

    #[test]
    fn test_pie_chart_handles_single_slice() {
        // Given: Data with only one segment (100%)
        let data = vec![ChartDataPoint::new("Everything", 100.0)];
        let chart = PieChart::new(data, ChartConfig::default());

        // When: Rendering
        let svg = chart.render();

        // Then: Should produce valid SVG with single slice
        assert!(svg.contains("<svg"), "Must produce valid SVG");
        assert!(svg.contains("100%"), "Should show 100% for single slice");
    }

    #[test]
    fn test_pie_chart_handles_all_zero_values() {
        // Given: Data with all zeros
        let data = vec![ChartDataPoint::new("A", 0.0), ChartDataPoint::new("B", 0.0)];
        let chart = PieChart::new(data, ChartConfig::default());

        // When: Rendering
        let svg = chart.render();

        // Then: Should show "no data" state (can't divide proportions)
        assert!(svg.contains("<svg"), "Must produce valid SVG");
        assert!(
            svg.contains("No data") || svg.contains("no data"),
            "All zeros should show no data message"
        );
    }

    // -------------------------------------------------------------------------
    // Helper Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_chart_data_point_new() {
        // Given/When: Creating a data point
        let point = ChartDataPoint::new("Test Label", 42.5);

        // Then: Fields should be set correctly
        assert_eq!(point.label, "Test Label");
        assert!((point.value - 42.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_chart_data_point_from_timestamp() {
        // Given: A timestamp
        let timestamp = chrono::Utc::now();

        // When: Creating a data point from timestamp
        let point = ChartDataPoint::from_timestamp(timestamp, 100.0);

        // Then: Label should be formatted date, value should be set
        assert!(!point.label.is_empty());
        assert!(
            point.label.contains('/'),
            "Label should be formatted as date"
        );
        assert!((point.value - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_chart_config_default() {
        // Given/When: Creating default config
        let config = ChartConfig::default();

        // Then: Should have sensible defaults
        assert!(config.width > 0);
        assert!(config.height > 0);
        assert!(!config.primary_color.is_empty());
        assert!(!config.background_color.is_empty());
    }

    #[test]
    fn test_escape_xml_special_characters() {
        // Given: String with special characters
        let input = "Test <script>alert('XSS')</script> & more";

        // When: Escaping
        let escaped = escape_xml(input);

        // Then: Special characters should be escaped
        assert!(!escaped.contains('<'));
        assert!(!escaped.contains('>'));
        assert!(!escaped.contains('&') || escaped.contains("&amp;") || escaped.contains("&lt;"));
        assert!(escaped.contains("&lt;"));
        assert!(escaped.contains("&gt;"));
    }
}
