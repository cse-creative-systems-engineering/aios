//! Optional, read-only GPU runtime adapter.
//!
//! The adapter is selected from a compiled-in catalog rather than generating
//! hardware-specific executable code.  Today that catalog contains the
//! `nvidia-smi` adapter; an unavailable command simply yields no observations.

use std::collections::BTreeMap;
use std::process::Command;

#[derive(Clone, Debug)]
pub enum GpuRuntimeAdapter {
    NvidiaSmi,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GpuDeviceSample {
    pub index: u32,
    pub uuid: String,
    pub name: String,
    pub temperature_c: Option<f64>,
    pub utilization_percent: Option<f64>,
    pub memory_used_mib: Option<f64>,
    pub memory_total_mib: Option<f64>,
    pub power_draw_w: Option<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GpuProcessSample {
    pub gpu_index: u32,
    pub pid: u32,
    pub memory_used_mib: Option<f64>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct GpuRuntimeSamples {
    pub devices: Vec<GpuDeviceSample>,
    pub processes: Vec<GpuProcessSample>,
}

impl GpuRuntimeAdapter {
    pub fn discover() -> Option<Self> {
        Command::new("nvidia-smi")
            .arg("--help")
            .output()
            .ok()
            .filter(|output| output.status.success())
            .map(|_| Self::NvidiaSmi)
    }

    pub fn collect(&self) -> Result<GpuRuntimeSamples, String> {
        match self {
            Self::NvidiaSmi => {
                let devices = run_nvidia_smi(
                    "--query-gpu=index,uuid,name,temperature.gpu,utilization.gpu,memory.used,memory.total,power.draw",
                )?;
                let processes =
                    run_nvidia_smi("--query-compute-apps=gpu_uuid,pid,used_gpu_memory")?;
                Ok(parse_nvidia_smi(&devices, &processes))
            }
        }
    }
}

fn run_nvidia_smi(query: &str) -> Result<String, String> {
    let output = Command::new("nvidia-smi")
        .args([query, "--format=csv,noheader,nounits"])
        .output()
        .map_err(|error| format!("nvidia-smi could not start: {error}"))?;
    if !output.status.success() {
        return Err(format!("nvidia-smi exited with {}", output.status));
    }
    String::from_utf8(output.stdout)
        .map_err(|error| format!("nvidia-smi returned non-UTF-8 output: {error}"))
}

pub fn parse_nvidia_smi(device_rows: &str, process_rows: &str) -> GpuRuntimeSamples {
    let devices = device_rows
        .lines()
        .filter_map(parse_device_row)
        .collect::<Vec<_>>();
    let indexes_by_uuid = devices
        .iter()
        .map(|device| (device.uuid.clone(), device.index))
        .collect::<BTreeMap<_, _>>();
    let processes = process_rows
        .lines()
        .filter_map(|row| parse_process_row(row, &indexes_by_uuid))
        .collect();
    GpuRuntimeSamples { devices, processes }
}

fn parse_device_row(row: &str) -> Option<GpuDeviceSample> {
    let values = csv_fields(row);
    Some(GpuDeviceSample {
        index: values.first()?.parse().ok()?,
        uuid: values.get(1)?.to_string(),
        name: values.get(2)?.to_string(),
        temperature_c: numeric(values.get(3)?),
        utilization_percent: numeric(values.get(4)?),
        memory_used_mib: numeric(values.get(5)?),
        memory_total_mib: numeric(values.get(6)?),
        power_draw_w: numeric(values.get(7)?),
    })
}

fn parse_process_row(
    row: &str,
    indexes_by_uuid: &BTreeMap<String, u32>,
) -> Option<GpuProcessSample> {
    let values = csv_fields(row);
    Some(GpuProcessSample {
        gpu_index: *indexes_by_uuid.get(values.first()?)?,
        pid: values.get(1)?.parse().ok()?,
        memory_used_mib: numeric(values.get(2)?),
    })
}

fn csv_fields(row: &str) -> Vec<String> {
    row.split(',')
        .map(|value| value.trim().to_string())
        .collect()
}

fn numeric(value: &str) -> Option<f64> {
    (!value.eq_ignore_ascii_case("n/a"))
        .then(|| value.parse().ok())
        .flatten()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_available_values_and_skips_na_values() {
        let samples = parse_nvidia_smi(
            "0, GPU-aaa, NVIDIA RTX, 71, 82, 2048, 8192, 125.50\n1, GPU-bbb, NVIDIA Other, N/A, 0, 0, 4096, N/A",
            "GPU-aaa, 4242, 512\nGPU-bbb, 2121, N/A",
        );
        assert_eq!(samples.devices.len(), 2);
        assert_eq!(samples.devices[0].temperature_c, Some(71.0));
        assert_eq!(samples.devices[1].temperature_c, None);
        assert_eq!(samples.processes[0].gpu_index, 0);
        assert_eq!(samples.processes[1].memory_used_mib, None);
    }
}
