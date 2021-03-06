use std::time::Instant;
use sysinfo::{ProcessorExt, System, SystemExt};

#[derive(Debug, Clone)]
pub struct CPUData {
	pub cpu_name: Box<str>,
	pub cpu_usage: f64,
}

#[derive(Debug, Clone)]
pub struct CPUPackage {
	pub cpu_vec: Vec<CPUData>,
	pub instant: Instant,
}

pub fn get_cpu_data_list(
	sys: &System, curr_time: &Instant,
) -> crate::utils::error::Result<CPUPackage> {
	let cpu_data = sys.get_processor_list();
	let mut cpu_vec = Vec::new();

	for cpu in cpu_data {
		cpu_vec.push(CPUData {
			cpu_name: Box::from(cpu.get_name()),
			cpu_usage: f64::from(cpu.get_cpu_usage()) * 100_f64,
		});
	}

	Ok(CPUPackage {
		cpu_vec,
		instant: *curr_time,
	})
}
