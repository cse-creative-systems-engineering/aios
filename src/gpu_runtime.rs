//! Optional, read-only GPU runtime adapter.
//!
//! The adapter is selected from a compiled-in catalog rather than generating
//! hardware-specific executable code.  Today that catalog contains the
//! `nvidia-smi` adapter plus Linux DRM/sysfs adapters for AMD and Intel. An
//! unavailable interface simply yields no observations.

use std::collections::BTreeMap;
use std::process::Command;

#[derive(Clone, Debug)]
pub enum GpuRuntimeAdapter {
    NvidiaSmi,
    LinuxDrmSysfs { vendor: GpuVendor },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GpuVendor {
    Amd,
    Intel,
}

impl GpuVendor {
    fn pci_id(self) -> &'static str {
        match self {
            Self::Amd => "0x1002",
            Self::Intel => "0x8086",
        }
    }

    fn key_prefix(self) -> &'static str {
        match self {
            Self::Amd => "amd-card",
            Self::Intel => "intel-card",
        }
    }

    fn display_name(self) -> &'static str {
        match self {
            Self::Amd => "AMD GPU",
            Self::Intel => "Intel GPU",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct GpuDeviceSample {
    /// Stable, presentation-safe key segment. NVIDIA preserves its familiar
    /// numeric index; DRM adapters use a vendor-qualified card name so two
    /// drivers cannot overwrite one another's observations.
    pub key: String,
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
    pub gpu_key: String,
    pub pid: u32,
    pub memory_used_mib: Option<f64>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct GpuRuntimeSamples {
    pub devices: Vec<GpuDeviceSample>,
    pub processes: Vec<GpuProcessSample>,
}

impl GpuRuntimeAdapter {
    pub fn discover() -> Vec<Self> {
        let mut adapters = Vec::new();
        if Command::new("nvidia-smi")
            .arg("--help")
            .output()
            .ok()
            .filter(|output| output.status.success())
            .is_some()
        {
            adapters.push(Self::NvidiaSmi);
        }
        for vendor in [GpuVendor::Amd, GpuVendor::Intel] {
            if linux_drm_has_vendor(vendor) {
                adapters.push(Self::LinuxDrmSysfs { vendor });
            }
        }
        adapters
    }

    pub fn source(&self) -> &'static str {
        match self {
            Self::NvidiaSmi => "nvidia_smi",
            Self::LinuxDrmSysfs {
                vendor: GpuVendor::Amd,
            } => "drm_sysfs_amd",
            Self::LinuxDrmSysfs {
                vendor: GpuVendor::Intel,
            } => "drm_sysfs_intel",
        }
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
            Self::LinuxDrmSysfs { vendor } => Ok(collect_linux_drm(*vendor)),
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
        .map(|device| (device.uuid.clone(), device.key.clone()))
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
        key: values.first()?.parse::<u32>().ok()?.to_string(),
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
    indexes_by_uuid: &BTreeMap<String, String>,
) -> Option<GpuProcessSample> {
    let values = csv_fields(row);
    Some(GpuProcessSample {
        gpu_key: indexes_by_uuid.get(values.first()?)?.clone(),
        pid: values.get(1)?.parse().ok()?,
        memory_used_mib: numeric(values.get(2)?),
    })
}

fn linux_drm_has_vendor(vendor: GpuVendor) -> bool {
    std::fs::read_dir("/sys/class/drm")
        .ok()
        .into_iter()
        .flatten()
        .flatten()
        .any(|entry| {
            is_card_name(&entry.file_name().to_string_lossy())
                && read_trimmed(entry.path().join("device/vendor"))
                    .is_some_and(|value| value.eq_ignore_ascii_case(vendor.pci_id()))
        })
}

fn collect_linux_drm(vendor: GpuVendor) -> GpuRuntimeSamples {
    let devices = std::fs::read_dir("/sys/class/drm")
        .ok()
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let card = entry.file_name().to_string_lossy().to_string();
            (is_card_name(&card)
                && read_trimmed(entry.path().join("device/vendor"))
                    .is_some_and(|value| value.eq_ignore_ascii_case(vendor.pci_id())))
            .then(|| sample_linux_drm_device(&entry.path(), &card, vendor))
        })
        .collect();
    GpuRuntimeSamples {
        devices,
        processes: Vec::new(),
    }
}

fn sample_linux_drm_device(
    card_path: &std::path::Path,
    card: &str,
    vendor: GpuVendor,
) -> GpuDeviceSample {
    let device = card_path.join("device");
    let hwmon = std::fs::read_dir(device.join("hwmon"))
        .ok()
        .into_iter()
        .flatten()
        .flatten()
        .next()
        .map(|entry| entry.path());
    let read_device = |name: &str| read_number(device.join(name));
    let read_hwmon = |name: &str| hwmon.as_ref().and_then(|path| read_number(path.join(name)));
    let slot = read_trimmed(device.join("uevent"))
        .and_then(|uevent| {
            uevent
                .lines()
                .find_map(|line| line.strip_prefix("PCI_SLOT_NAME=").map(str::to_owned))
        })
        .unwrap_or_else(|| card.to_string());
    linux_drm_sample(
        card,
        &slot,
        vendor,
        read_hwmon("temp1_input"),
        read_device("gpu_busy_percent"),
        read_device("mem_info_vram_used"),
        read_device("mem_info_vram_total"),
        read_hwmon("power1_average"),
    )
}

fn linux_drm_sample(
    card: &str,
    slot: &str,
    vendor: GpuVendor,
    temperature_millic: Option<f64>,
    utilization_percent: Option<f64>,
    memory_used_bytes: Option<f64>,
    memory_total_bytes: Option<f64>,
    power_microwatts: Option<f64>,
) -> GpuDeviceSample {
    GpuDeviceSample {
        key: format!("{}{}", vendor.key_prefix(), &card[4..]),
        uuid: slot.into(),
        name: vendor.display_name().into(),
        temperature_c: temperature_millic.map(|value| value / 1000.0),
        utilization_percent,
        memory_used_mib: memory_used_bytes.map(bytes_to_mib),
        memory_total_mib: memory_total_bytes.map(bytes_to_mib),
        power_draw_w: power_microwatts.map(|value| value / 1_000_000.0),
    }
}

fn is_card_name(name: &str) -> bool {
    name.strip_prefix("card").is_some_and(|suffix| {
        !suffix.is_empty() && suffix.chars().all(|char| char.is_ascii_digit())
    })
}

fn read_trimmed(path: impl AsRef<std::path::Path>) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|value| value.trim().to_string())
}

fn read_number(path: impl AsRef<std::path::Path>) -> Option<f64> {
    read_trimmed(path)?.parse().ok()
}

fn bytes_to_mib(value: f64) -> f64 {
    value / (1024.0 * 1024.0)
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
        assert_eq!(samples.processes[0].gpu_key, "0");
        assert_eq!(samples.processes[1].memory_used_mib, None);
    }

    #[test]
    fn accepts_only_real_drm_card_names() {
        assert!(is_card_name("card0"));
        assert!(is_card_name("card42"));
        assert!(!is_card_name("card0-DP-1"));
        assert!(!is_card_name("renderD128"));
    }

    #[test]
    fn converts_sysfs_units_without_inventing_values() {
        assert_eq!(bytes_to_mib(8.0 * 1024.0 * 1024.0), 8.0);
        assert_eq!(GpuVendor::Amd.key_prefix(), "amd-card");
        assert_eq!(GpuVendor::Intel.display_name(), "Intel GPU");
    }

    #[test]
    fn drm_fixture_uses_vendor_qualified_keys_and_optional_metrics() {
        let sample = linux_drm_sample(
            "card2",
            "0000:05:00.0",
            GpuVendor::Amd,
            Some(67_500.0),
            Some(73.0),
            Some(4.0 * 1024.0 * 1024.0),
            None,
            None,
        );
        assert_eq!(sample.key, "amd-card2");
        assert_eq!(sample.uuid, "0000:05:00.0");
        assert_eq!(sample.temperature_c, Some(67.5));
        assert_eq!(sample.utilization_percent, Some(73.0));
        assert_eq!(sample.memory_used_mib, Some(4.0));
        assert_eq!(sample.memory_total_mib, None);
    }
}
