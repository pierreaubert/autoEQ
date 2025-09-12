use clap::Parser;
use ndarray::Array1;
use plotly::{
	common::{ColorScale, ColorScalePalette, Marker, Mode, Title},
	contour::Contour,
	Layout, Plot, Scatter,
};
use std::fs;
use std::path::Path;

// Import environment utilities
use autoeq_env::{get_data_generated_dir, get_records_dir};

// Import the test functions and metadata
use autoeq_testfunctions::{get_function_metadata, FunctionMetadata};

// Import shared function registry
use autoeq_de::function_registry::TestFunction;

/// CLI arguments for plotting test functions
#[derive(Parser)]
#[command(name = "plot_functions")]
#[command(about = "Plot test functions using contour plots with Plotly")]
struct Args {
	/// Height of the plot in pixels
	#[arg(short = 'H', long, default_value = "800")]
	height: usize,

	/// Width of the plot in pixels
	#[arg(short = 'W', long, default_value = "800")]
	width: usize,

	/// Number of points along x-axis
	#[arg(short = 'x', long, default_value = "100")]
	xn: usize,

	/// Number of points along y-axis
	#[arg(short = 'y', long, default_value = "100")]
	yn: usize,

	/// X-axis bounds (min,max)
	#[arg(long, default_value = "-5.0,5.0")]
	x_bounds: String,

	/// Y-axis bounds (min,max)
	#[arg(long, default_value = "-5.0,5.0")]
	y_bounds: String,

	/// Output directory for HTML files
	#[arg(short, long)]
	output_dir: Option<String>,

	/// List of specific functions to plot (comma-separated), if empty plots all
	#[arg(short, long)]
	functions: Option<String>,

	/// Directory containing CSV files with optimization traces
	#[arg(long)]
	csv_dir: Option<String>,

	/// Show optimization traces from CSV files
	#[arg(long)]
	show_traces: bool,

	/// Use function metadata for bounds (overrides x_bounds and y_bounds)
	#[arg(long)]
	use_metadata: bool,

	/// Create convergence plots showing loss function vs iterations/evaluations
	#[arg(long)]
	convergence_plots: bool,
}

// TestFunction type now imported from shared function_registry

#[derive(Debug, Clone)]
struct OptimizationPoint {
	iteration: usize,
	x: Vec<f64>,
	best_result: f64,
	f_value: f64,
	is_improvement: bool,
}

#[derive(Debug, Clone)]
struct OptimizationTrace {
	function_name: String,
	points: Vec<OptimizationPoint>,
}

fn read_csv_trace(csv_path: &str) -> Result<OptimizationTrace, Box<dyn std::error::Error>> {
	let content = fs::read_to_string(csv_path)?;
	let lines: Vec<&str> = content.trim().split('\n').collect();

	if lines.len() < 2 {
		return Err("CSV file must have at least header and one data row".into());
	}

	let header = lines[0];

	// Determine CSV format based on header
	let is_new_format = header.starts_with("eval_id,generation,");
	let is_old_format = header.starts_with("iteration,");

	if !is_new_format && !is_old_format {
		return Err(format!(
            "Invalid CSV header format. Expected to start with 'eval_id,generation,' or 'iteration,', got: {}",
            header
        )
        .into());
	}

	// Extract function name from filename
	let raw_name = Path::new(csv_path).file_stem().and_then(|s| s.to_str()).unwrap_or("unknown");

	// Clean function name by removing _block_XXXX suffix if present
	let function_name = if raw_name.contains("_block_") {
		raw_name.split("_block_").next().unwrap_or(raw_name).to_string()
	} else {
		raw_name.to_string()
	};

	let mut points = Vec::new();

	for (line_idx, line) in lines.iter().skip(1).enumerate() {
		let parts: Vec<&str> = line.split(',').collect();

		if parts.len() < 4 {
			return Err(format!("Line {}: insufficient columns", line_idx + 2).into());
		}

		let point = if is_new_format {
			// New format: eval_id,generation,x0,x1,f_value,best_so_far,is_improvement
			if parts.len() < 7 {
				return Err(format!(
					"Line {}: insufficient columns for new format (expected 7+)",
					line_idx + 2
				)
				.into());
			}

			let eval_id: usize =
				parts[0].parse().map_err(|_| format!("Line {}: invalid eval_id", line_idx + 2))?;

			let _generation: usize = parts[1]
				.parse()
				.map_err(|_| format!("Line {}: invalid generation", line_idx + 2))?;

			// Parse x coordinates (between generation and f_value/best_so_far/is_improvement)
			let x_end = parts.len() - 3; // f_value, best_so_far, is_improvement
			let mut x = Vec::new();
			for i in 2..x_end {
				let coord: f64 = parts[i].parse().map_err(|_| {
					format!("Line {}: invalid x coordinate at column {}", line_idx + 2, i)
				})?;
				x.push(coord);
			}

			if x.len() != 2 {
				return Err(format!(
					"Line {}: expected 2D coordinates, got {}D",
					line_idx + 2,
					x.len()
				)
				.into());
			}

			let f_value: f64 = parts[x_end]
				.parse()
				.map_err(|_| format!("Line {}: invalid f_value", line_idx + 2))?;

			let best_so_far: f64 = parts[x_end + 1]
				.parse()
				.map_err(|_| format!("Line {}: invalid best_so_far", line_idx + 2))?;

			let is_improvement: bool = parts[x_end + 2]
				.parse()
				.map_err(|_| format!("Line {}: invalid is_improvement", line_idx + 2))?;

			OptimizationPoint {
				iteration: eval_id,
				x,
				best_result: best_so_far,
				f_value,
				is_improvement,
			}
		} else {
			// Old format: iteration,x0,x1,best_result,convergence,is_improvement
			let iteration: usize = parts[0]
				.parse()
				.map_err(|_| format!("Line {}: invalid iteration number", line_idx + 2))?;

			// Parse x coordinates (all columns between iteration and last 3 columns)
			let x_columns_end = parts.len() - 3; // best_result, convergence, is_improvement
			let mut x = Vec::new();

			for i in 1..x_columns_end {
				let coord: f64 = parts[i].parse().map_err(|_| {
					format!("Line {}: invalid x coordinate at column {}", line_idx + 2, i)
				})?;
				x.push(coord);
			}

			if x.len() != 2 {
				return Err(format!(
					"Line {}: expected 2D coordinates, got {}D",
					line_idx + 2,
					x.len()
				)
				.into());
			}

			let best_result: f64 = parts[x_columns_end]
				.parse()
				.map_err(|_| format!("Line {}: invalid best_result", line_idx + 2))?;

			let is_improvement: bool = parts[x_columns_end + 2]
				.parse()
				.map_err(|_| format!("Line {}: invalid is_improvement", line_idx + 2))?;

			OptimizationPoint {
				iteration,
				x,
				best_result,
				f_value: best_result, // In old format, we don't have separate f_value
				is_improvement,
			}
		};

		points.push(point);
	}

	Ok(OptimizationTrace { function_name, points })
}

fn find_csv_for_function(csv_dir: &str, function_name: &str) -> Vec<String> {
	autoeq_de::function_registry::find_csv_files_for_function(csv_dir, function_name)
}

fn add_optimization_trace(
	plot: &mut Plot,
	trace: &OptimizationTrace,
	x_bounds: (f64, f64),
	y_bounds: (f64, f64),
) {
	if trace.points.is_empty() {
		return;
	}

	// Filter points to only those within bounds
	let valid_points: Vec<&OptimizationPoint> = trace
		.points
		.iter()
		.filter(|point| {
			point.x.len() >= 2
				&& point.x[0] >= x_bounds.0
				&& point.x[0] <= x_bounds.1
				&& point.x[1] >= y_bounds.0
				&& point.x[1] <= y_bounds.1
		})
		.collect();

	eprintln!("  Found {} valid points", valid_points.len());

	if valid_points.is_empty() {
		return;
	}

	// Split points into improvements and non-improvements
	let improvements: Vec<&OptimizationPoint> =
		valid_points.iter().filter(|point| point.is_improvement).copied().collect();

	let non_improvements: Vec<&OptimizationPoint> =
		valid_points.iter().filter(|point| !point.is_improvement).copied().collect();

	// Plot all evaluation points (gray)
	if !non_improvements.is_empty() {
		let x_coords: Vec<f64> = non_improvements.iter().map(|p| p.x[0]).collect();
		let y_coords: Vec<f64> = non_improvements.iter().map(|p| p.x[1]).collect();

		let trace_all =
			Scatter::new(x_coords, y_coords).mode(Mode::Markers).name("Evaluations").marker(
				Marker::new()
					.color("rgba(128, 128, 128, 0.6)") // Gray with transparency
					.size(4)
					.symbol(plotly::common::MarkerSymbol::Circle),
			);
		plot.add_trace(trace_all);
	}

	// Plot improvement points (bright colors on Viridis-friendly colors)
	if !improvements.is_empty() {
		let x_coords: Vec<f64> = improvements.iter().map(|p| p.x[0]).collect();
		let y_coords: Vec<f64> = improvements.iter().map(|p| p.x[1]).collect();

		let trace_improvements =
			Scatter::new(x_coords, y_coords).mode(Mode::Markers).name("Improvements").marker(
				Marker::new()
					.color("rgba(255, 255, 0, 0.9)") // Bright yellow - highly visible on Viridis
					.size(8)
					.line(
						plotly::common::Line::new()
							.color("rgba(255, 140, 0, 1.0)") // Orange border
							.width(2.0),
					)
					.symbol(plotly::common::MarkerSymbol::Circle),
			);
		plot.add_trace(trace_improvements);
	}

	// Plot the optimization path (connecting improvements)
	if improvements.len() > 1 {
		let x_coords: Vec<f64> = improvements.iter().map(|p| p.x[0]).collect();
		let y_coords: Vec<f64> = improvements.iter().map(|p| p.x[1]).collect();

		let path_trace =
			Scatter::new(x_coords, y_coords).mode(Mode::Lines).name("Optimization Path").line(
				plotly::common::Line::new()
					.color("rgba(255, 140, 0, 0.8)") // Orange line
					.width(2.0)
					.dash(plotly::common::DashType::Dash),
			);
		plot.add_trace(path_trace);
	}

	// Highlight the best point (final solution)
	if let Some(best_point) = improvements.last() {
		let best_trace = Scatter::new(vec![best_point.x[0]], vec![best_point.x[1]])
			.mode(Mode::Markers)
			.name("Best Solution")
			.marker(
				Marker::new()
					.color("rgba(255, 0, 0, 1.0)") // Bright red - stands out on any background
					.size(12)
					.line(
						plotly::common::Line::new()
							.color("rgba(255, 255, 255, 1.0)") // White border
							.width(3.0),
					)
					.symbol(plotly::common::MarkerSymbol::Star),
			);
		plot.add_trace(best_trace);
	}
}

/// Create a convergence plot showing loss function over iterations/evaluations
fn plot_convergence(trace: &OptimizationTrace, output_dir: &str, width: usize, height: usize) {
	if trace.points.is_empty() {
		eprintln!("  Warning: No data points for convergence plot");
		return;
	}

	let iterations: Vec<usize> = trace.points.iter().map(|p| p.iteration).collect();
	let best_results: Vec<f64> = trace.points.iter().map(|p| p.best_result).collect();
	let f_values: Vec<f64> = trace.points.iter().map(|p| p.f_value).collect();

	// Create best-so-far trace (shows the convergence of the optimization)
	let best_trace =
		Scatter::new(iterations.clone(), best_results).mode(Mode::Lines).name("Best So Far").line(
			plotly::common::Line::new()
				.color("rgba(0, 100, 200, 0.8)") // Blue
				.width(3.0),
		);

	// Create function evaluation trace (shows all individual evaluations)
	// Only plot every nth point to avoid overcrowding for large datasets
	let step_size = std::cmp::max(1, trace.points.len() / 1000); // Limit to ~1000 points max
	let sampled_iterations: Vec<usize> = iterations.iter().step_by(step_size).copied().collect();
	let sampled_f_values: Vec<f64> = f_values.iter().step_by(step_size).copied().collect();

	let eval_trace = Scatter::new(sampled_iterations, sampled_f_values)
		.mode(Mode::Markers)
		.name("Function Evaluations")
		.marker(
			Marker::new()
				.color("rgba(200, 200, 200, 0.4)") // Light gray with transparency
				.size(2)
				.symbol(plotly::common::MarkerSymbol::Circle),
		);

	let layout = Layout::new()
		.title(Title::with_text(&format!("Convergence: {}", trace.function_name)))
		.width(width)
		.height(height)
		.x_axis(
			plotly::layout::Axis::new()
				.title(Title::with_text("Iteration/Evaluation"))
				.type_(plotly::layout::AxisType::Linear),
		)
		.y_axis(
			plotly::layout::Axis::new()
				.title(Title::with_text("Function Value"))
				.type_(plotly::layout::AxisType::Log), // Use log scale for better visualization
		)
		.legend(plotly::layout::Legend::new().x(0.7).y(0.9));

	let mut plot = Plot::new();
	plot.add_trace(eval_trace);
	plot.add_trace(best_trace);
	plot.set_layout(layout);

	// Use a clean function name for the convergence plot (remove _block_XXXX suffix if present)
	let clean_name = if trace.function_name.contains("_block_") {
		trace.function_name.split("_block_").next().unwrap_or(&trace.function_name)
	} else {
		&trace.function_name
	};
	let filename = format!("{}/{}_convergence.html", output_dir, clean_name.replace(' ', "_"));
	plot.write_html(&filename);
	println!("  Created convergence plot: {}", filename);
}

fn main() {
	let args = Args::parse();

	// Set default directories if not provided, using environment-based paths
	let output_dir = args.output_dir.unwrap_or_else(|| match get_data_generated_dir() {
		Ok(data_dir) => {
			let mut path = data_dir;
			path.push("plot_functions");
			path.to_string_lossy().to_string()
		}
		Err(e) => {
			eprintln!("Error accessing data directory: {}", e);
			eprintln!("Please set AUTOEQ_DIR environment variable to your AutoEQ project root.");
			std::process::exit(1);
		}
	});

	let csv_dir = args.csv_dir.unwrap_or_else(|| match get_records_dir() {
		Ok(records_dir) => records_dir.to_string_lossy().to_string(),
		Err(e) => {
			eprintln!("Error accessing records directory: {}", e);
			eprintln!("Please set AUTOEQ_DIR environment variable to your AutoEQ project root.");
			std::process::exit(1);
		}
	});

	// Parse bounds
	let x_bounds = parse_bounds(&args.x_bounds).expect("Invalid x_bounds format");
	let y_bounds = parse_bounds(&args.y_bounds).expect("Invalid y_bounds format");

	// Create output directory
	std::fs::create_dir_all(&output_dir).expect("Failed to create output directory");

	// Get all test functions and metadata
	let functions = get_test_functions();
	let metadata = get_function_metadata();

	// Filter functions if specific ones are requested
	let functions_to_plot = if let Some(func_names) = &args.functions {
		let requested: Vec<&str> = func_names.split(',').map(|s| s.trim()).collect();
		functions.into_iter().filter(|(name, _)| requested.contains(&name.as_str())).collect()
	} else {
		functions
	};

	println!("Plotting {} functions with {}x{} grid", functions_to_plot.len(), args.xn, args.yn);

	// Plot each function
	for (name, func) in functions_to_plot {
		println!("Plotting function: {}", name);

		// Check if function requires more than 2D (skip if so)
		if let Some(meta) = metadata.get(&name) {
			if meta.bounds.len() > 2 {
				println!(
					"  Skipping '{}': requires {}D input, plotting only supports 2D",
					name,
					meta.bounds.len()
				);
				continue;
			}
		}

		// Determine bounds to use
		let (plot_x_bounds, plot_y_bounds) = if args.use_metadata {
			if let Some(meta) = metadata.get(&name) {
				// Use metadata bounds if available
				if meta.bounds.len() >= 2 {
					(meta.bounds[0], meta.bounds[1])
				} else {
					// Fallback to CLI bounds if metadata doesn't have enough dimensions
					eprintln!("  Warning: Function '{}' metadata has insufficient bounds, using CLI bounds", name);
					(x_bounds, y_bounds)
				}
			} else {
				eprintln!("  Warning: No metadata found for function '{}', using CLI bounds", name);
				(x_bounds, y_bounds)
			}
		} else {
			// Use CLI-provided bounds
			(x_bounds, y_bounds)
		};

		println!(
			"  Using bounds: x=({}, {}), y=({}, {})",
			plot_x_bounds.0, plot_x_bounds.1, plot_y_bounds.0, plot_y_bounds.1
		);

		// Load optimization trace if requested
		let trace = if args.show_traces || args.convergence_plots {
			let csv_files = find_csv_for_function(&csv_dir, &name);
			if !csv_files.is_empty() {
				// Read and combine all block files
				let mut combined_trace = OptimizationTrace {
					function_name: name.clone(),
					points: Vec::new(),
				};
				
				for csv_path in &csv_files {
					match read_csv_trace(csv_path) {
						Ok(mut trace) => {
							combined_trace.points.append(&mut trace.points);
						}
						Err(e) => {
							eprintln!("  Warning: Failed to load trace from {}: {}", csv_path, e);
						}
					}
				}
				
				if !combined_trace.points.is_empty() {
					println!("  Loaded optimization trace with {} points from {} file(s)", 
						combined_trace.points.len(), csv_files.len());
					Some(combined_trace)
				} else {
					println!("  No valid trace data found in {} file(s)", csv_files.len());
					None
				}
			} else {
				println!("  No trace file found for function '{}'", name);
				None
			}
		} else {
			None
		};

		plot_function(
			&name,
			func,
			plot_x_bounds,
			plot_y_bounds,
			args.xn,
			args.yn,
			args.width,
			args.height,
			&output_dir,
			if args.show_traces { trace.as_ref() } else { None },
			metadata.get(&name),
		);

		// Create convergence plot if requested and trace is available
		if args.convergence_plots {
			if let Some(ref trace) = trace {
				plot_convergence(trace, &output_dir, args.width, args.height);
			}
		}
	}

	println!("Plots saved to directory: {}", output_dir);
}

fn parse_bounds(bounds_str: &str) -> Result<(f64, f64), Box<dyn std::error::Error>> {
	// Remove possible surrounding single or double quotes
	let cleaned = bounds_str.trim_matches(|c| c == '\'' || c == '"');

	// Try splitting by comma or whitespace
	let parts: Vec<&str> = if cleaned.contains(',') {
		cleaned.split(',').collect()
	} else {
		cleaned.split_whitespace().collect()
	};

	if parts.len() != 2 {
		return Err("Bounds must be in format 'min,max' or 'min max'".into());
	}

	eprintln!("  bounds {}  {}", parts[0].trim(), parts[1].trim());
	let min = parts[0].trim().parse::<f64>()?;
	let max = parts[1].trim().parse::<f64>()?;
	Ok((min, max))
}

fn plot_function(
	name: &str,
	func: TestFunction,
	x_bounds: (f64, f64),
	y_bounds: (f64, f64),
	xn: usize,
	yn: usize,
	width: usize,
	height: usize,
	output_dir: &str,
	trace: Option<&OptimizationTrace>,
	metadata: Option<&FunctionMetadata>,
) {
	// Create coordinate grids
	let x_vals: Vec<f64> = (0..xn)
		.map(|i| x_bounds.0 + (x_bounds.1 - x_bounds.0) * i as f64 / (xn - 1) as f64)
		.collect();

	let y_vals: Vec<f64> = (0..yn)
		.map(|i| y_bounds.0 + (y_bounds.1 - y_bounds.0) * i as f64 / (yn - 1) as f64)
		.collect();

	// Evaluate function on grid
	let mut z_vals = Vec::with_capacity(yn);
	for &y in &y_vals {
		let mut row = Vec::with_capacity(xn);
		for &x in &x_vals {
			let input = Array1::from(vec![x, y]);
			let z = func(&input);
			row.push(z);
		}
		z_vals.push(row);
	}

	// Create contour plot with custom colorbar configuration
	// Using fraction mode with 60% height
	let contour = Contour::new(x_vals.clone(), y_vals.clone(), z_vals.clone())
		.color_scale(ColorScale::Palette(ColorScalePalette::Viridis))
		.color_bar(
			plotly::common::ColorBar::new()
				.len_mode(plotly::common::ThicknessMode::Pixels)
				.len(60 * height / 100) // 60% in fraction mode (may need to be scaled)
				.y_anchor(plotly::common::Anchor::Bottom)
				.y(0.0), // Position at bottom
		);

	// Create layout
	let layout = Layout::new()
		.title(Title::with_text(&format!("Function: {}", name)))
		.width(width)
		.height(height)
		.x_axis(plotly::layout::Axis::new().title(Title::with_text("X")))
		.y_axis(plotly::layout::Axis::new().title(Title::with_text("Y")));

	// Create plot and add contour
	let mut plot = Plot::new();
	plot.add_trace(contour);

	// Add optimization trace if available
	if let Some(trace) = trace {
		add_optimization_trace(&mut plot, trace, x_bounds, y_bounds);
	}

	// Add global minima if metadata is available
	if let Some(meta) = metadata {
		add_global_minima(&mut plot, meta, x_bounds, y_bounds);

		// Add constraint boundaries if present
		if !meta.inequality_constraints.is_empty() {
			add_constraint_boundaries(&mut plot, meta, x_bounds, y_bounds, &x_vals, &y_vals);
		}
	}

	plot.set_layout(layout);

	let filename = format!("{}/{}.html", output_dir, name.replace(' ', "_"));
	plot.write_html(&filename);
}

/// Automatically get all test functions using the shared registry
fn get_test_functions() -> Vec<(String, TestFunction)> {
	let registry = autoeq_de::function_registry::FunctionRegistry::new();
	let metadata = get_function_metadata();
	let mut functions = Vec::new();

	// Build function list from registry and metadata
	for (name, _meta) in metadata.iter() {
		if let Some(func) = registry.get(name) {
			functions.push((name.clone(), func));
		} else {
			eprintln!(
				"Warning: Function '{}' found in metadata but not in registry",
				name
			);
		}
	}

	// Sort functions alphabetically for consistent ordering
	functions.sort_by(|a, b| a.0.cmp(&b.0));

	eprintln!("Discovered {} plottable functions from registry", functions.len());
	functions
}

// Function mapping now handled by shared FunctionRegistry

/// Add global minima markers to the plot
fn add_global_minima(
	plot: &mut Plot,
	metadata: &FunctionMetadata,
	x_bounds: (f64, f64),
	y_bounds: (f64, f64),
) {
	let valid_minima: Vec<&(Vec<f64>, f64)> = metadata
		.global_minima
		.iter()
		.filter(|(coords, _)| {
			coords.len() >= 2
				&& coords[0] >= x_bounds.0
				&& coords[0] <= x_bounds.1
				&& coords[1] >= y_bounds.0
				&& coords[1] <= y_bounds.1
		})
		.collect();

	if !valid_minima.is_empty() {
		let x_coords: Vec<f64> = valid_minima.iter().map(|(coords, _)| coords[0]).collect();
		let y_coords: Vec<f64> = valid_minima.iter().map(|(coords, _)| coords[1]).collect();

		let global_minima_trace =
			Scatter::new(x_coords, y_coords).mode(Mode::Markers).name("Global Minima").marker(
				Marker::new()
					.color("rgba(255, 255, 255, 1.0)") // White center
					.size(10)
					.line(
						plotly::common::Line::new()
							.color("rgba(255, 0, 255, 1.0)") // Magenta border
							.width(3.0),
					)
					.symbol(plotly::common::MarkerSymbol::Diamond),
			);
		plot.add_trace(global_minima_trace);
	}
}

/// Add constraint boundary visualization to the plot
fn add_constraint_boundaries(
	plot: &mut Plot,
	metadata: &FunctionMetadata,
	_x_bounds: (f64, f64),
	_y_bounds: (f64, f64),
	x_vals: &[f64],
	y_vals: &[f64],
) {
	// Create a contour for each constraint showing feasible/infeasible regions
	for (i, constraint_fn) in metadata.inequality_constraints.iter().enumerate() {
		let mut constraint_vals = Vec::with_capacity(y_vals.len());

		for &y in y_vals {
			let mut row = Vec::with_capacity(x_vals.len());
			for &x in x_vals {
				let input = Array1::from(vec![x, y]);
				let constraint_value = constraint_fn(&input);
				row.push(constraint_value);
			}
			constraint_vals.push(row);
		}

		// Add contour line at constraint_value = 0 (boundary)
		let constraint_contour = Contour::new(x_vals.to_vec(), y_vals.to_vec(), constraint_vals)
			.show_scale(false) // Don't show colorbar for constraints
			.contours(
				plotly::contour::Contours::new().start(0.0).end(0.0).size(1.0), // Only show the boundary line
			)
			.line(
				plotly::common::Line::new()
					.color(format!("rgba(255, 0, 0, 0.8)")) // Red constraint boundary
					.width(3.0)
					.dash(plotly::common::DashType::Dash),
			)
			.name(&format!("Constraint {}", i + 1))
			.hover_info(plotly::common::HoverInfo::Skip); // Don't show hover info for constraints

		plot.add_trace(constraint_contour);
	}
}
