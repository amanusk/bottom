use crate::{
	app, constants,
	data_conversion::{ConvertedCpuData, ConvertedProcessData},
	utils::{error, gen_util::*},
};
use std::cmp::max;
use tui::{
	backend,
	layout::{Alignment, Constraint, Direction, Layout, Rect},
	style::{Color, Modifier, Style},
	terminal::Frame,
	widgets::{Axis, Block, Borders, Chart, Dataset, Marker, Paragraph, Row, Table, Text, Widget},
	Terminal,
};

const TEXT_COLOUR: Color = Color::Gray;
const GRAPH_COLOUR: Color = Color::Gray;
const BORDER_STYLE_COLOUR: Color = Color::Gray;
const HIGHLIGHTED_BORDER_STYLE_COLOUR: Color = Color::LightBlue;
const GOLDEN_RATIO: f32 = 0.618_034; // Approx, good enough for use (also Clippy gets mad if it's too long)

// Headers
const CPU_LEGEND_HEADER: [&str; 2] = ["CPU", "Use%"];
const DISK_HEADERS: [&str; 7] = ["Disk", "Mount", "Used", "Free", "Total", "R/s", "W/s"];
const TEMP_HEADERS: [&str; 2] = ["Sensor", "Temp"];
const NON_WINDOWS_NETWORK_HEADERS: [&str; 4] = ["RX", "TX", "Total RX", "Total TX"];
const WINDOWS_NETWORK_HEADERS: [&str; 2] = ["RX", "TX"];
const FORCE_MIN_THRESHOLD: usize = 5;

lazy_static! {
	static ref HELP_TEXT: [Text<'static>; 15] = [
		Text::raw("\nGeneral Keybindings\n"),
		Text::raw("q, Ctrl-c to quit.\n"),
		Text::raw("Ctrl-r to reset all data.\n"),
		Text::raw("f to toggle freezing and unfreezing the display.\n"),
		Text::raw(
			"Ctrl+Up/k, Ctrl+Down/j, Ctrl+Left/h, Ctrl+Right/l to navigate between panels.\n"
		),
		Text::raw("Up and Down scrolls through a list.\n"),
		Text::raw("Esc to close a dialog window (help or dd confirmation).\n"),
		Text::raw("? to get this help screen.\n"),
		Text::raw("\n Process Panel Keybindings\n"),
		Text::raw("dd to kill the selected process.\n"),
		Text::raw("c to sort by CPU usage.\n"),
		Text::raw("m to sort by memory usage.\n"),
		Text::raw("p to sort by PID.\n"),
		Text::raw("n to sort by process name.\n"),
		Text::raw("`Tab` to group together processes with the same name.\n")
	];
	static ref COLOUR_LIST: Vec<Color> = gen_n_colours(constants::NUM_COLOURS);
	static ref CANVAS_BORDER_STYLE: Style = Style::default().fg(BORDER_STYLE_COLOUR);
	static ref CANVAS_HIGHLIGHTED_BORDER_STYLE: Style =
		Style::default().fg(HIGHLIGHTED_BORDER_STYLE_COLOUR);
	static ref DISK_HEADERS_LENS: Vec<usize> = DISK_HEADERS
		.iter()
		.map(|entry| max(FORCE_MIN_THRESHOLD, entry.len()))
		.collect::<Vec<_>>();
	static ref CPU_LEGEND_HEADER_LENS: Vec<usize> = CPU_LEGEND_HEADER
		.iter()
		.map(|entry| max(FORCE_MIN_THRESHOLD, entry.len()))
		.collect::<Vec<_>>();
	static ref TEMP_HEADERS_LENS: Vec<usize> = TEMP_HEADERS
		.iter()
		.map(|entry| max(FORCE_MIN_THRESHOLD, entry.len()))
		.collect::<Vec<_>>();
	static ref NON_WINDOWS_NETWORK_HEADERS_LENS: Vec<usize> = NON_WINDOWS_NETWORK_HEADERS
		.iter()
		.map(|entry| max(FORCE_MIN_THRESHOLD, entry.len()))
		.collect::<Vec<_>>();
	static ref WINDOWS_NETWORK_HEADERS_LENS: Vec<usize> = WINDOWS_NETWORK_HEADERS
		.iter()
		.map(|entry| max(FORCE_MIN_THRESHOLD, entry.len()))
		.collect::<Vec<_>>();
}

#[derive(Default)]
pub struct CanvasData {
	pub rx_display: String,
	pub tx_display: String,
	pub total_rx_display: String,
	pub total_tx_display: String,
	pub network_data_rx: Vec<(f64, f64)>,
	pub network_data_tx: Vec<(f64, f64)>,
	pub disk_data: Vec<Vec<String>>,
	pub temp_sensor_data: Vec<Vec<String>>,
	pub process_data: Vec<ConvertedProcessData>,
	pub grouped_process_data: Vec<ConvertedProcessData>,
	pub memory_labels: Vec<(u64, u64)>,
	pub mem_data: Vec<(f64, f64)>,
	pub swap_data: Vec<(f64, f64)>,
	pub cpu_data: Vec<ConvertedCpuData>,
}

/// Generates random colours.
/// Strategy found from https://martin.ankerl.com/2009/12/09/how-to-create-random-colors-programmatically/
fn gen_n_colours(num_to_gen: i32) -> Vec<Color> {
	fn gen_hsv(h: f32) -> f32 {
		let new_val = h + GOLDEN_RATIO;
		if new_val > 1.0 {
			new_val.fract()
		} else {
			new_val
		}
	}
	/// This takes in an h, s, and v value of range [0, 1]
	/// For explanation of what this does, see
	/// https://en.wikipedia.org/wiki/HSL_and_HSV#HSV_to_RGB_alternative
	fn hsv_to_rgb(hue: f32, saturation: f32, value: f32) -> (u8, u8, u8) {
		fn hsv_helper(num: u32, hu: f32, sat: f32, val: f32) -> f32 {
			let k = (num as f32 + hu * 6.0) % 6.0;
			val - val * sat * float_max(float_min(k, float_min(4.1 - k, 1.1)), 0.0)
		}

		(
			(hsv_helper(5, hue, saturation, value) * 255.0) as u8,
			(hsv_helper(3, hue, saturation, value) * 255.0) as u8,
			(hsv_helper(1, hue, saturation, value) * 255.0) as u8,
		)
	}

	// Generate colours
	let mut colour_vec: Vec<Color> = vec![
		Color::LightCyan,
		Color::LightYellow,
		Color::Red,
		Color::Green,
		Color::LightMagenta,
	];

	let mut h: f32 = 0.4; // We don't need random colours... right?
	for _i in 0..num_to_gen {
		h = gen_hsv(h);
		let result = hsv_to_rgb(h, 0.5, 0.95);
		colour_vec.push(Color::Rgb(result.0, result.1, result.2));
	}

	colour_vec
}

pub fn draw_data<B: backend::Backend>(
	terminal: &mut Terminal<B>, app_state: &mut app::App,
) -> error::Result<()> {
	terminal.autoresize()?;
	terminal.draw(|mut f| {
		if app_state.show_help {
			// Only for the help
			let vertical_dialog_chunk = Layout::default()
				.direction(Direction::Vertical)
				.margin(1)
				.constraints(
					[
						Constraint::Percentage(32),
						Constraint::Percentage(40),
						Constraint::Percentage(28),
					]
					.as_ref(),
				)
				.split(f.size());

			let middle_dialog_chunk = Layout::default()
				.direction(Direction::Horizontal)
				.margin(0)
				.constraints(
					[
						Constraint::Percentage(30),
						Constraint::Percentage(40),
						Constraint::Percentage(30),
					]
					.as_ref(),
				)
				.split(vertical_dialog_chunk[1]);

			Paragraph::new(HELP_TEXT.iter())
				.block(
					Block::default()
						.title("Help (Press Esc to close)")
						.borders(Borders::ALL),
				)
				.style(Style::default().fg(Color::Gray))
				.alignment(Alignment::Left)
				.wrap(true)
				.render(&mut f, middle_dialog_chunk[1]);
		} else if app_state.show_dd {
			let vertical_dialog_chunk = Layout::default()
				.direction(Direction::Vertical)
				.margin(1)
				.constraints(
					[
						Constraint::Percentage(40),
						Constraint::Percentage(20),
						Constraint::Percentage(40),
					]
					.as_ref(),
				)
				.split(f.size());

			let middle_dialog_chunk = Layout::default()
				.direction(Direction::Horizontal)
				.margin(0)
				.constraints(
					[
						Constraint::Percentage(30),
						Constraint::Percentage(40),
						Constraint::Percentage(30),
					]
					.as_ref(),
				)
				.split(vertical_dialog_chunk[1]);

			if let Some(dd_err) = app_state.dd_err.clone() {
				let dd_text = [Text::raw(format!(
					"\nFailure to properly kill the process - {}",
					dd_err
				))];

				Paragraph::new(dd_text.iter())
					.block(
						Block::default()
							.title("Kill Process Error (Press Esc to close)")
							.borders(Borders::ALL),
					)
					.style(Style::default().fg(Color::Gray))
					.alignment(Alignment::Center)
					.wrap(true)
					.render(&mut f, middle_dialog_chunk[1]);
			} else if let Some(process_list) = app_state.get_current_highlighted_process_list() {
				if let Some(process) = process_list.first() {
					let dd_text = [
					if app_state.is_grouped() {
						Text::raw(format!(
							"\nAre you sure you want to kill {} process(es) with name {}?",
							process_list.len(), process.name
						))
					} else {
						Text::raw(format!(
							"\nAre you sure you want to kill process {} with PID {}?",
							process.name, process.pid
						))
					},
					Text::raw("\n\nPress ENTER to proceed, ESC to exit."),
					Text::raw("\nNote that if bottom is frozen, it must be unfrozen for changes to be shown."),
				];

					Paragraph::new(dd_text.iter())
						.block(
							Block::default()
								.title("Kill Process Confirmation (Press Esc to close)")
								.borders(Borders::ALL),
						)
						.style(Style::default().fg(Color::Gray))
						.alignment(Alignment::Center)
						.wrap(true)
						.render(&mut f, middle_dialog_chunk[1]);
				} else {
					app_state.show_dd = false;
				}
			} else {
				// This is a bit nasty, but it works well... I guess.
				app_state.show_dd = false;
			}
		} else {
			let vertical_chunks = Layout::default()
				.direction(Direction::Vertical)
				.margin(1)
				.constraints(
					[
						Constraint::Percentage(33),
						Constraint::Percentage(34),
						Constraint::Percentage(34),
					]
					.as_ref(),
				)
				.split(f.size());

			let middle_chunks = Layout::default()
				.direction(Direction::Horizontal)
				.margin(0)
				.constraints([Constraint::Percentage(60), Constraint::Percentage(40)].as_ref())
				.split(vertical_chunks[1]);

			let middle_divided_chunk_2 = Layout::default()
				.direction(Direction::Vertical)
				.margin(0)
				.constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
				.split(middle_chunks[1]);

			let bottom_chunks = Layout::default()
				.direction(Direction::Horizontal)
				.margin(0)
				.constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
				.split(vertical_chunks[2]);

			// Component specific chunks
			let cpu_chunk = Layout::default()
				.direction(Direction::Horizontal)
				.margin(0)
				.constraints(
					if app_state.left_legend {
						[Constraint::Percentage(15), Constraint::Percentage(85)]
					} else {
						[Constraint::Percentage(85), Constraint::Percentage(15)]
					}
					.as_ref(),
				)
				.split(vertical_chunks[0]);

			let network_chunk = Layout::default()
				.direction(Direction::Vertical)
				.margin(0)
				.constraints(
					if app_state.left_legend {
						[Constraint::Percentage(10), Constraint::Percentage(90)]
					} else {
						[Constraint::Percentage(75), Constraint::Percentage(10)]
					}
					.as_ref(),
				)
				.split(bottom_chunks[0]);

			// Default chunk index based on left or right legend setting
			let legend_index = if app_state.left_legend { 0 } else { 1 };
			let graph_index = if app_state.left_legend { 1 } else { 0 };

			// Set up blocks and their components
			// CPU graph
			draw_cpu_graph(&mut f, &app_state, cpu_chunk[graph_index]);

			// CPU legend
			draw_cpu_legend(&mut f, app_state, cpu_chunk[legend_index]);

			//Memory usage graph
			draw_memory_graph(&mut f, &app_state, middle_chunks[0]);

			// Network graph
			draw_network_graph(&mut f, &app_state, network_chunk[0]);

			draw_network_labels(&mut f, app_state, network_chunk[1]);

			// Temperature table
			draw_temp_table(&mut f, app_state, middle_divided_chunk_2[0]);

			// Disk usage table
			draw_disk_table(&mut f, app_state, middle_divided_chunk_2[1]);

			// Processes table
			draw_processes_table(&mut f, app_state, bottom_chunks[1]);
		}
	})?;

	Ok(())
}

fn draw_cpu_graph<B: backend::Backend>(f: &mut Frame<B>, app_state: &app::App, draw_loc: Rect) {
	let cpu_data: &[ConvertedCpuData] = &app_state.canvas_data.cpu_data;

	// CPU usage graph
	let x_axis: Axis<String> = Axis::default()
		.style(Style::default().fg(GRAPH_COLOUR))
		.bounds([0.0, constants::TIME_STARTS_FROM as f64 * 10.0]);
	let y_axis = Axis::default()
		.style(Style::default().fg(GRAPH_COLOUR))
		.bounds([-0.5, 100.5])
		.labels(&["0%", "100%"]);

	let mut dataset_vector: Vec<Dataset> = Vec::new();
	let mut cpu_entries_vec: Vec<(Style, Vec<(f64, f64)>)> = Vec::new();

	for (i, cpu) in cpu_data.iter().enumerate() {
		let mut avg_cpu_exist_offset = 0;
		if app_state.show_average_cpu {
			if i == 0 {
				// Skip, we want to render the average cpu last!
				continue;
			} else {
				avg_cpu_exist_offset = 1;
			}
		}

		cpu_entries_vec.push((
			Style::default().fg(COLOUR_LIST[(i - avg_cpu_exist_offset) % COLOUR_LIST.len()]),
			cpu.cpu_data
				.iter()
				.map(<(f64, f64)>::from)
				.collect::<Vec<_>>(),
		));
	}

	if app_state.show_average_cpu {
		if let Some(avg_cpu_entry) = cpu_data.first() {
			cpu_entries_vec.push((
				Style::default().fg(COLOUR_LIST[(cpu_data.len() - 1) % COLOUR_LIST.len()]),
				avg_cpu_entry
					.cpu_data
					.iter()
					.map(<(f64, f64)>::from)
					.collect::<Vec<_>>(),
			));
		}
	}

	for cpu_entry in &cpu_entries_vec {
		dataset_vector.push(
			Dataset::default()
				.marker(if app_state.use_dot {
					Marker::Dot
				} else {
					Marker::Braille
				})
				.style(cpu_entry.0)
				.data(&(cpu_entry.1)),
		);
	}

	Chart::default()
		.block(
			Block::default()
				.title("CPU")
				.borders(Borders::ALL)
				.border_style(match app_state.current_application_position {
					app::ApplicationPosition::Cpu => *CANVAS_HIGHLIGHTED_BORDER_STYLE,
					_ => *CANVAS_BORDER_STYLE,
				}),
		)
		.x_axis(x_axis)
		.y_axis(y_axis)
		.datasets(&dataset_vector)
		.render(f, draw_loc);
}

fn draw_cpu_legend<B: backend::Backend>(
	f: &mut Frame<B>, app_state: &mut app::App, draw_loc: Rect,
) {
	let cpu_data: &[ConvertedCpuData] = &(app_state.canvas_data.cpu_data);

	let num_rows = i64::from(draw_loc.height) - 5;
	let start_position = get_start_position(
		num_rows,
		&(app_state.scroll_direction),
		&mut app_state.previous_cpu_table_position,
		app_state.currently_selected_cpu_table_position,
	);

	let sliced_cpu_data = (&cpu_data[start_position as usize..]).to_vec();
	let mut stringified_cpu_data: Vec<Vec<String>> = Vec::new();

	for cpu in sliced_cpu_data {
		if let Some(cpu_data) = cpu.cpu_data.last() {
			stringified_cpu_data.push(vec![
				cpu.cpu_name.clone(),
				format!("{:.0}%", cpu_data.usage.round()),
			]);
		}
	}

	let mut cpu_row_counter = 0;

	let cpu_rows = stringified_cpu_data
		.iter()
		.enumerate()
		.map(|(itx, cpu_string_row)| {
			Row::StyledData(
				cpu_string_row.iter(),
				match app_state.current_application_position {
					app::ApplicationPosition::Cpu => {
						if cpu_row_counter
							== app_state.currently_selected_cpu_table_position - start_position
						{
							cpu_row_counter = -1;
							Style::default().fg(Color::Black).bg(Color::Cyan)
						} else {
							if cpu_row_counter >= 0 {
								cpu_row_counter += 1;
							}
							Style::default().fg(COLOUR_LIST[itx % COLOUR_LIST.len()])
						}
					}
					_ => Style::default().fg(COLOUR_LIST[itx % COLOUR_LIST.len()]),
				},
			)
		});

	// Calculate widths
	let width = f64::from(draw_loc.width);
	let width_ratios = vec![0.5, 0.5];
	let variable_intrinsic_results =
		get_variable_intrinsic_widths(width as u16, &width_ratios, &CPU_LEGEND_HEADER_LENS);
	let intrinsic_widths: Vec<u16> =
		((variable_intrinsic_results.0)[0..variable_intrinsic_results.1]).to_vec();

	// Draw
	Table::new(CPU_LEGEND_HEADER.iter(), cpu_rows)
		.block(Block::default().borders(Borders::ALL).border_style(
			match app_state.current_application_position {
				app::ApplicationPosition::Cpu => *CANVAS_HIGHLIGHTED_BORDER_STYLE,
				_ => *CANVAS_BORDER_STYLE,
			},
		))
		.header_style(Style::default().fg(Color::LightBlue))
		.widths(
			&(intrinsic_widths
				.into_iter()
				.map(|calculated_width| Constraint::Length(calculated_width as u16))
				.collect::<Vec<_>>()),
		)
		.render(f, draw_loc);
}

fn _draw_memory_table<B: backend::Backend>(
	_f: &mut Frame<B>, _app_state: &app::App, _draw_loc: Rect,
) {
        // TODO: Memory table to be made for basic mode
}

fn draw_memory_graph<B: backend::Backend>(f: &mut Frame<B>, app_state: &app::App, draw_loc: Rect) {
	let mem_data: &[(f64, f64)] = &(app_state.canvas_data.mem_data);
	let swap_data: &[(f64, f64)] = &(app_state.canvas_data.swap_data);
	let memory_labels: &[(u64, u64)] = &(app_state.canvas_data.memory_labels);

	let x_axis: Axis<String> = Axis::default()
		.style(Style::default().fg(GRAPH_COLOUR))
		.bounds([0.0, constants::TIME_STARTS_FROM as f64 * 10.0]);
	let y_axis = Axis::default()
		.style(Style::default().fg(GRAPH_COLOUR))
		.bounds([-0.5, 100.5]) // Offset as the zero value isn't drawn otherwise...
		.labels(&["0%", "100%"]);

	let mem_name = "RAM:".to_string()
		+ &format!(
			"{:3}%",
			(mem_data.last().unwrap_or(&(0_f64, 0_f64)).1.round() as u64)
		) + &format!(
		"   {:.1}GB/{:.1}GB",
		memory_labels.first().unwrap_or(&(0, 0)).0 as f64 / 1024.0,
		memory_labels.first().unwrap_or(&(0, 0)).1 as f64 / 1024.0
	);
	let swap_name: String;

	let mut mem_canvas_vec: Vec<Dataset> = vec![Dataset::default()
		.name(&mem_name)
		.marker(if app_state.use_dot {
			Marker::Dot
		} else {
			Marker::Braille
		})
		.style(Style::default().fg(COLOUR_LIST[0]))
		.data(&mem_data)];

	if !(&swap_data).is_empty() {
		if let Some(last_canvas_result) = (&swap_data).last() {
			if last_canvas_result.1 >= 0.0 {
				swap_name = "SWP:".to_string()
					+ &format!(
						"{:3}%",
						(swap_data.last().unwrap_or(&(0_f64, 0_f64)).1.round() as u64)
					) + &format!(
					"   {:.1}GB/{:.1}GB",
					memory_labels[1].0 as f64 / 1024.0,
					memory_labels[1].1 as f64 / 1024.0
				);
				mem_canvas_vec.push(
					Dataset::default()
						.name(&swap_name)
						.marker(if app_state.use_dot {
							Marker::Dot
						} else {
							Marker::Braille
						})
						.style(Style::default().fg(COLOUR_LIST[1]))
						.data(&swap_data),
				);
			}
		}
	}

	Chart::default()
		.block(
			Block::default()
				.title("Memory")
				.borders(Borders::ALL)
				.border_style(match app_state.current_application_position {
					app::ApplicationPosition::Mem => *CANVAS_HIGHLIGHTED_BORDER_STYLE,
					_ => *CANVAS_BORDER_STYLE,
				}),
		)
		.x_axis(x_axis)
		.y_axis(y_axis)
		.datasets(&mem_canvas_vec)
		.render(f, draw_loc);
}

fn draw_network_graph<B: backend::Backend>(f: &mut Frame<B>, app_state: &app::App, draw_loc: Rect) {
	let network_data_rx: &[(f64, f64)] = &(app_state.canvas_data.network_data_rx);
	let network_data_tx: &[(f64, f64)] = &(app_state.canvas_data.network_data_tx);

	let x_axis: Axis<String> = Axis::default()
		.style(Style::default().fg(GRAPH_COLOUR))
		.bounds([0.0, 600_000.0]);
	let y_axis = Axis::default()
		.style(Style::default().fg(GRAPH_COLOUR))
		.bounds([-0.5, 30_f64])
		.labels(&["0B", "1KiB", "1MiB", "1GiB"]);
	Chart::default()
		.block(
			Block::default()
				.title("Network")
				.borders(Borders::ALL)
				.border_style(match app_state.current_application_position {
					app::ApplicationPosition::Network => *CANVAS_HIGHLIGHTED_BORDER_STYLE,
					_ => *CANVAS_BORDER_STYLE,
				}),
		)
		.x_axis(x_axis)
		.y_axis(y_axis)
		.datasets(&[
			Dataset::default()
				.marker(if app_state.use_dot {
					Marker::Dot
				} else {
					Marker::Braille
				})
				.style(Style::default().fg(COLOUR_LIST[0]))
				.data(&network_data_rx),
			Dataset::default()
				.marker(if app_state.use_dot {
					Marker::Dot
				} else {
					Marker::Braille
				})
				.style(Style::default().fg(COLOUR_LIST[1]))
				.data(&network_data_tx),
		])
		.render(f, draw_loc);
}

fn draw_network_labels<B: backend::Backend>(
	f: &mut Frame<B>, app_state: &mut app::App, draw_loc: Rect,
) {
	let rx_display: String = app_state.canvas_data.rx_display.clone();
	let tx_display: String = app_state.canvas_data.tx_display.clone();
	let total_rx_display: String = app_state.canvas_data.total_rx_display.clone();
	let total_tx_display: String = app_state.canvas_data.total_tx_display.clone();

	// Gross but I need it to work...
	let total_network = if cfg!(not(target_os = "windows")) {
		vec![vec![
			rx_display,
			tx_display,
			total_rx_display,
			total_tx_display,
		]]
	} else {
		vec![vec![rx_display, tx_display]]
	};
	let mapped_network = total_network.iter().map(|val| Row::Data(val.iter()));

	// Calculate widths
	let width_ratios: Vec<f64>;
	let lens: &Vec<usize>;
	let width = f64::from(draw_loc.width);

	if cfg!(not(target_os = "windows")) {
		width_ratios = vec![0.25, 0.25, 0.25, 0.25];
		lens = &NON_WINDOWS_NETWORK_HEADERS_LENS;
	} else {
		width_ratios = vec![0.25, 0.25];
		lens = &WINDOWS_NETWORK_HEADERS_LENS;
	}
	let variable_intrinsic_results =
		get_variable_intrinsic_widths(width as u16, &width_ratios, lens);
	let intrinsic_widths: Vec<u16> =
		((variable_intrinsic_results.0)[0..variable_intrinsic_results.1]).to_vec();

	// Draw
	Table::new(
		// TODO: [OPT] Feels like I can optimize this and avoid multiple to_vec calls?
		if cfg!(not(target_os = "windows")) {
			NON_WINDOWS_NETWORK_HEADERS.to_vec()
		} else {
			WINDOWS_NETWORK_HEADERS.to_vec()
		}
		.iter(),
		mapped_network,
	)
	.block(Block::default().borders(Borders::ALL).border_style(
		match app_state.current_application_position {
			app::ApplicationPosition::Network => *CANVAS_HIGHLIGHTED_BORDER_STYLE,
			_ => *CANVAS_BORDER_STYLE,
		},
	))
	.header_style(Style::default().fg(Color::LightBlue))
	.widths(
		&(intrinsic_widths
			.into_iter()
			.map(|calculated_width| Constraint::Length(calculated_width as u16))
			.collect::<Vec<_>>()),
	)
	.render(f, draw_loc);
}

fn draw_temp_table<B: backend::Backend>(
	f: &mut Frame<B>, app_state: &mut app::App, draw_loc: Rect,
) {
	let temp_sensor_data: &[Vec<String>] = &(app_state.canvas_data.temp_sensor_data);

	let num_rows = i64::from(draw_loc.height) - 5;
	let start_position = get_start_position(
		num_rows,
		&(app_state.scroll_direction),
		&mut app_state.previous_temp_position,
		app_state.currently_selected_temperature_position,
	);

	let sliced_vec: Vec<Vec<String>> = (&temp_sensor_data[start_position as usize..]).to_vec();
	let mut temp_row_counter = 0;

	let temperature_rows = sliced_vec.iter().map(|temp_row| {
		Row::StyledData(
			temp_row.iter(),
			match app_state.current_application_position {
				app::ApplicationPosition::Temp => {
					if temp_row_counter
						== app_state.currently_selected_temperature_position - start_position
					{
						temp_row_counter = -1;
						Style::default().fg(Color::Black).bg(Color::Cyan)
					} else {
						if temp_row_counter >= 0 {
							temp_row_counter += 1;
						}
						Style::default().fg(TEXT_COLOUR)
					}
				}
				_ => Style::default().fg(TEXT_COLOUR),
			},
		)
	});

	// Calculate widths
	let width = f64::from(draw_loc.width);
	let width_ratios = [0.5, 0.5];
	let variable_intrinsic_results =
		get_variable_intrinsic_widths(width as u16, &width_ratios, &TEMP_HEADERS_LENS);
	let intrinsic_widths: Vec<u16> =
		((variable_intrinsic_results.0)[0..variable_intrinsic_results.1]).to_vec();

	// Draw
	Table::new(TEMP_HEADERS.iter(), temperature_rows)
		.block(
			Block::default()
				.title("Temperatures")
				.borders(Borders::ALL)
				.border_style(match app_state.current_application_position {
					app::ApplicationPosition::Temp => *CANVAS_HIGHLIGHTED_BORDER_STYLE,
					_ => *CANVAS_BORDER_STYLE,
				}),
		)
		.header_style(Style::default().fg(Color::LightBlue))
		.widths(
			&(intrinsic_widths
				.into_iter()
				.map(|calculated_width| Constraint::Length(calculated_width as u16))
				.collect::<Vec<_>>()),
		)
		.render(f, draw_loc);
}

fn draw_disk_table<B: backend::Backend>(
	f: &mut Frame<B>, app_state: &mut app::App, draw_loc: Rect,
) {
	let disk_data: &[Vec<String>] = &(app_state.canvas_data.disk_data);
	let num_rows = i64::from(draw_loc.height) - 5;
	let start_position = get_start_position(
		num_rows,
		&(app_state.scroll_direction),
		&mut app_state.previous_disk_position,
		app_state.currently_selected_disk_position,
	);

	let sliced_vec: Vec<Vec<String>> = (&disk_data[start_position as usize..]).to_vec();
	let mut disk_counter = 0;

	let disk_rows = sliced_vec.iter().map(|disk| {
		Row::StyledData(
			disk.iter(),
			match app_state.current_application_position {
				app::ApplicationPosition::Disk => {
					if disk_counter == app_state.currently_selected_disk_position - start_position {
						disk_counter = -1;
						Style::default().fg(Color::Black).bg(Color::Cyan)
					} else {
						if disk_counter >= 0 {
							disk_counter += 1;
						}
						Style::default().fg(TEXT_COLOUR)
					}
				}
				_ => Style::default().fg(TEXT_COLOUR),
			},
		)
	});

	// Calculate widths
	// TODO: Ellipsis on strings?
	let width = f64::from(draw_loc.width);
	let width_ratios = [0.2, 0.15, 0.13, 0.13, 0.13, 0.13, 0.13];
	let variable_intrinsic_results =
		get_variable_intrinsic_widths(width as u16, &width_ratios, &DISK_HEADERS_LENS);
	let intrinsic_widths: Vec<u16> =
		((variable_intrinsic_results.0)[0..variable_intrinsic_results.1]).to_vec();

	// Draw!
	Table::new(DISK_HEADERS.iter(), disk_rows)
		.block(
			Block::default()
				.title("Disk")
				.borders(Borders::ALL)
				.border_style(match app_state.current_application_position {
					app::ApplicationPosition::Disk => *CANVAS_HIGHLIGHTED_BORDER_STYLE,
					_ => *CANVAS_BORDER_STYLE,
				}),
		)
		.header_style(
			Style::default()
				.fg(Color::LightBlue)
				.modifier(Modifier::BOLD),
		)
		.widths(
			&(intrinsic_widths
				.into_iter()
				.map(|calculated_width| Constraint::Length(calculated_width as u16))
				.collect::<Vec<_>>()),
		)
		.render(f, draw_loc);
}

fn draw_processes_table<B: backend::Backend>(
	f: &mut Frame<B>, app_state: &mut app::App, draw_loc: Rect,
) {
	let process_data: &[ConvertedProcessData] = if app_state.is_grouped() {
		&app_state.canvas_data.grouped_process_data
	} else {
		&app_state.canvas_data.process_data
	};

	// Admittedly this is kinda a hack... but we need to:
	// * Scroll
	// * Show/hide elements based on scroll position
	// As such, we use a process_counter to know when we've hit the process we've currently scrolled to.  We also need to move the list - we can
	// do so by hiding some elements!
	let num_rows = i64::from(draw_loc.height) - 5;

	let start_position = get_start_position(
		num_rows,
		&(app_state.scroll_direction),
		&mut app_state.previous_process_position,
		app_state.currently_selected_process_position,
	);

	let sliced_vec: Vec<ConvertedProcessData> = (&process_data[start_position as usize..]).to_vec();
	let mut process_counter = 0;

	// Draw!
	let process_rows = sliced_vec.iter().map(|process| {
		let stringified_process_vec: Vec<String> = vec![
			if app_state.is_grouped() {
				process.group.len().to_string()
			} else {
				process.pid.to_string()
			},
			process.name.clone(),
			process.cpu_usage.clone(),
			process.mem_usage.clone(),
		];
		Row::StyledData(
			stringified_process_vec.into_iter(),
			match app_state.current_application_position {
				app::ApplicationPosition::Process => {
					if process_counter
						== app_state.currently_selected_process_position - start_position
					{
						process_counter = -1;
						Style::default().fg(Color::Black).bg(Color::Cyan)
					} else {
						if process_counter >= 0 {
							process_counter += 1;
						}
						Style::default().fg(TEXT_COLOUR)
					}
				}
				_ => Style::default().fg(TEXT_COLOUR),
			},
		)
	});

	use app::data_collection::processes::ProcessSorting;
	let mut pid_or_name = if app_state.is_grouped() {
		"Count"
	} else {
		"PID(p)"
	}
	.to_string();
	let mut name = "Name(n)".to_string();
	let mut cpu = "CPU%(c)".to_string();
	let mut mem = "Mem%(m)".to_string();

	let direction_val = if app_state.process_sorting_reverse {
		"⯆".to_string()
	} else {
		"⯅".to_string()
	};

	match app_state.process_sorting_type {
		ProcessSorting::CPU => cpu += &direction_val,
		ProcessSorting::MEM => mem += &direction_val,
		ProcessSorting::PID => pid_or_name += &direction_val,
		ProcessSorting::NAME => name += &direction_val,
	};

	// TODO: [OPT] Reuse calculation to save time?
	let process_headers = [pid_or_name, name, cpu, mem];
	let process_headers_lens: Vec<usize> = process_headers
		.iter()
		.map(|entry| entry.len())
		.collect::<Vec<_>>();

	// Calculate widths
	let width = f64::from(draw_loc.width);
	let width_ratios = [0.2, 0.4, 0.2, 0.2];
	let variable_intrinsic_results =
		get_variable_intrinsic_widths(width as u16, &width_ratios, &process_headers_lens);
	let intrinsic_widths: Vec<u16> =
		((variable_intrinsic_results.0)[0..variable_intrinsic_results.1]).to_vec();

	Table::new(process_headers.iter(), process_rows)
		.block(
			Block::default()
				.title("Processes")
				.borders(Borders::ALL)
				.border_style(match app_state.current_application_position {
					app::ApplicationPosition::Process => *CANVAS_HIGHLIGHTED_BORDER_STYLE,
					_ => *CANVAS_BORDER_STYLE,
				}),
		)
		.header_style(Style::default().fg(Color::LightBlue))
		.widths(
			&(intrinsic_widths
				.into_iter()
				.map(|calculated_width| Constraint::Length(calculated_width as u16))
				.collect::<Vec<_>>()),
		)
		.render(f, draw_loc);
}

/// A somewhat jury-rigged solution to simulate a variable intrinsic layout for
/// table widths.  Note that this will do one main pass to try to properly
/// allocate widths.  This will thus potentially cut off latter elements
/// (return size of 0) if it is too small (threshold), but will try its best.
///
/// `width thresholds` and `desired_widths_ratio` should be the same length.
/// Otherwise bad things happen.
fn get_variable_intrinsic_widths(
	total_width: u16, desired_widths_ratio: &[f64], width_thresholds: &[usize],
) -> (Vec<u16>, usize) {
	let num_widths = desired_widths_ratio.len();
	let mut resulting_widths: Vec<u16> = vec![0; num_widths];
	let mut last_index = 0;

	let mut remaining_width = (total_width - (num_widths as u16 - 1)) as i32; // Required for spaces...
	let desired_widths = desired_widths_ratio
		.iter()
		.map(|&desired_width_ratio| (desired_width_ratio * total_width as f64) as i32)
		.collect::<Vec<_>>();

	for (itx, desired_width) in desired_widths.into_iter().enumerate() {
		resulting_widths[itx] = if desired_width < width_thresholds[itx] as i32 {
			// Try to take threshold, else, 0
			if remaining_width < width_thresholds[itx] as i32 {
				0
			} else {
				remaining_width -= width_thresholds[itx] as i32;
				width_thresholds[itx] as u16
			}
		} else {
			// Take as large as possible
			if remaining_width < desired_width {
				// Check the biggest chunk possible
				if remaining_width < width_thresholds[itx] as i32 {
					0
				} else {
					let temp_width = remaining_width;
					remaining_width = 0;
					temp_width as u16
				}
			} else {
				remaining_width -= desired_width;
				desired_width as u16
			}
		};

		if resulting_widths[itx] == 0 {
			break;
		} else {
			last_index += 1;
		}
	}

	// Simple redistribution tactic - if there's any space left, split it evenly amongst all members
	if last_index < num_widths {
		let for_all_widths = (remaining_width / last_index as i32) as u16;
		let mut remainder = remaining_width % last_index as i32;

		for resulting_width in &mut resulting_widths {
			*resulting_width += for_all_widths;
			if remainder > 0 {
				*resulting_width += 1;
				remainder -= 1;
			}
		}
	}

	(resulting_widths, last_index)
}

fn get_start_position(
	num_rows: i64, scroll_direction: &app::ScrollDirection, previously_scrolled_position: &mut i64,
	currently_selected_position: i64,
) -> i64 {
	match scroll_direction {
		app::ScrollDirection::DOWN => {
			if currently_selected_position < *previously_scrolled_position + num_rows {
				// If, using previous_scrolled_position, we can see the element
				// (so within that and + num_rows) just reuse the current previously scrolled position
				*previously_scrolled_position
			} else if currently_selected_position >= num_rows {
				// Else if the current position past the last element visible in the list, omit
				// until we can see that element
				*previously_scrolled_position = currently_selected_position - num_rows;
				currently_selected_position - num_rows
			} else {
				// Else, if it is not past the last element visible, do not omit anything
				0
			}
		}
		app::ScrollDirection::UP => {
			if currently_selected_position <= *previously_scrolled_position {
				// If it's past the first element, then show from that element downwards
				*previously_scrolled_position = currently_selected_position;
				currently_selected_position
			} else {
				// Else, don't change what our start position is from whatever it is set to!
				*previously_scrolled_position
			}
		}
	}
}
