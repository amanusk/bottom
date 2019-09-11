use heim_common::prelude::StreamExt;
use std::time::Instant;

#[derive(Clone, Default)]
pub struct DiskData {
	pub name : Box<str>,
	pub mount_point : Box<str>,
	pub free_space : u64,
	pub used_space : u64,
	pub total_space : u64,
}

#[derive(Clone)]
pub struct IOData {
	pub mount_point : Box<str>,
	pub read_bytes : u64,
	pub write_bytes : u64,
	pub instant : Instant,
}

pub async fn get_io_usage_list(get_physical : bool) -> Result<Vec<IOData>, heim::Error> {
	let mut io_list : Vec<IOData> = Vec::new();
	if get_physical {
		let mut physical_counter_stream = heim::disk::io_counters_physical();
		while let Some(io) = physical_counter_stream.next().await {
			let io = io?;
			io_list.push(IOData {
				mount_point : Box::from(io.device_name().to_str().unwrap_or("Name Unavailable")),
				read_bytes : io.read_bytes().get::<heim_common::units::information::megabyte>(),
				write_bytes : io.write_bytes().get::<heim_common::units::information::megabyte>(),
				instant : Instant::now(),
			})
		}
	}
	else {
		let mut counter_stream = heim::disk::io_counters();
		while let Some(io) = counter_stream.next().await {
			let io = io?;
			io_list.push(IOData {
				mount_point : Box::from(io.device_name().to_str().unwrap_or("Name Unavailable")),
				read_bytes : io.read_bytes().get::<heim_common::units::information::megabyte>(),
				write_bytes : io.write_bytes().get::<heim_common::units::information::megabyte>(),
				instant : Instant::now(),
			})
		}
	}

	Ok(io_list)
}

pub async fn get_disk_usage_list() -> Result<Vec<DiskData>, heim::Error> {
	let mut vec_disks : Vec<DiskData> = Vec::new();
	let mut partitions_stream = heim::disk::partitions_physical();

	while let Some(part) = partitions_stream.next().await {
		let partition = part?; // TODO: Change this?  We don't want to error out immediately...
		let usage = heim::disk::usage(partition.mount_point().to_path_buf()).await?;

		vec_disks.push(DiskData {
			free_space : usage.free().get::<heim_common::units::information::megabyte>(),
			used_space : usage.used().get::<heim_common::units::information::megabyte>(),
			total_space : usage.total().get::<heim_common::units::information::megabyte>(),
			mount_point : Box::from(partition.mount_point().to_str().unwrap_or("Name Unavailable")),
			name : Box::from(partition.device().unwrap_or_else(|| std::ffi::OsStr::new("Name Unavailable")).to_str().unwrap_or("Name Unavailable")),
		});
	}

	Ok(vec_disks)
}
