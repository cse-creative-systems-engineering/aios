use crate::capability::PrincipalId;
use crate::graph::{
    EdgeId, EdgeMetadata, EdgeProvenance, EdgeType, NodeId, NodeMetadata, NodeType,
    ProvenanceSource, SystemGraph, TrustLevel,
};
use crate::protocol::{Duration, EventType, HealthState, Timestamp, now};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum DiscoveryError {
    RootMissing(PathBuf),
    ReadFailed { path: String, source: std::io::Error },
}

impl std::fmt::Display for DiscoveryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DiscoveryError::RootMissing(root) => {
                write!(f, "no sysfs or procfs tree at {}", root.display())
            }
            DiscoveryError::ReadFailed { path, source } => {
                write!(f, "failed to read {path}: {source}")
            }
        }
    }
}

impl std::error::Error for DiscoveryError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiscoveryEvent {
    pub event_type: EventType,
    pub node_id: NodeId,
    pub timestamp: Timestamp,
}

fn is_dynamic(node_type: &NodeType) -> bool {
    matches!(node_type, NodeType::Device | NodeType::Bus | NodeType::Sensor)
}

/// Standard sysfs attribute names that drivers use to expose the loaded
/// firmware revision. Probed generically on every device directory; Aios does
/// not assume any particular driver or system.
const FIRMWARE_ATTRIBUTES: &[&str] = &[
    "firmware",
    "firmware_name",
    "firmware_version",
    "firmware_rev",
    "fw_ver",
    "fw_version",
    "ucode",
];

/// Control files in `sys/class/firmware` that are not firmware entries.
const FIRMWARE_CLASS_CONTROL_FILES: &[&str] = &["timeout"];

pub struct DiscoveryOptions {
    pub root: PathBuf,
    pub now: Timestamp,
    pub ttl: Duration,
}

impl Default for DiscoveryOptions {
    fn default() -> Self {
        Self {
            root: PathBuf::from("/"),
            now: now(),
            ttl: 60,
        }
    }
}

pub struct SysfsDiscovery {
    options: DiscoveryOptions,
}

impl Default for SysfsDiscovery {
    fn default() -> Self {
        Self::new()
    }
}

impl SysfsDiscovery {
    pub fn new() -> Self {
        Self {
            options: DiscoveryOptions::default(),
        }
    }

    pub fn with_options(options: DiscoveryOptions) -> Self {
        Self { options }
    }

    pub fn scan(&self) -> Result<SystemGraph, DiscoveryError> {
        let root = &self.options.root;
        if !root.join("sys").exists() && !root.join("proc").exists() {
            return Err(DiscoveryError::RootMissing(root.clone()));
        }
        let t = self.options.now;
        let expires = t.checked_add(self.options.ttl);
        let mut graph = SystemGraph::new();

        self.discover_kernel(root, &mut graph, t, expires)?;
        self.discover_drivers(root, &mut graph, t, expires)?;
        self.discover_cpu(root, &mut graph, t, expires)?;
        self.discover_memory(root, &mut graph, t, expires)?;
        self.discover_network(root, &mut graph, t, expires)?;
        self.discover_pci(root, &mut graph, t, expires)?;
        self.discover_usb(root, &mut graph, t, expires)?;
        self.discover_firmware(root, &mut graph, t, expires)?;
        self.discover_block(root, &mut graph, t, expires)?;
        self.discover_filesystems(root, &mut graph, t, expires)?;
        self.discover_sensors(root, &mut graph, t, expires)?;
        self.discover_processes(root, &mut graph, t, expires)?;
        // After physical devices exist, link each network interface to its
        // underlying PCI/USB device so the interface inherits the device's
        // driver/firmware/bus dependencies (M6 acceptance criterion #6).
        self.link_network_interfaces(root, &mut graph, t);
        Ok(graph)
    }

    /// Post-pass: for each `device:net-<iface>`, resolve the underlying
    /// PCI/USB device via `sys/class/net/<iface>/device` and add a
    /// `depends_on` edge to it. Runs after PCI/USB discovery so the target
    /// nodes exist.
    fn link_network_interfaces(
        &self,
        root: &Path,
        graph: &mut SystemGraph,
        t: Timestamp,
    ) {
        let nodes: Vec<NodeId> = graph
            .nodes()
            .values()
            .filter(|n| n.node_type == NodeType::Device && n.node_id.0.starts_with("device:net-"))
            .map(|n| n.node_id.clone())
            .collect();
        for iface_id in nodes {
            let name = iface_id.0.trim_start_matches("device:net-");
            let Some(slot) = self.underlying_device_slot(root, &format!("sys/class/net/{name}/device")) else {
                continue;
            };
            for physical in [NodeId(format!("device:pci-{slot}")), NodeId(format!("device:usb-{slot}"))] {
                if graph.get_node(&physical).is_some() {
                    self.add_depends_on(graph, &iface_id, &physical, t);
                    break;
                }
            }
        }
    }

    pub fn reconcile(
        &self,
        graph: &mut SystemGraph,
    ) -> Result<Vec<DiscoveryEvent>, DiscoveryError> {
        let fresh = self.scan()?;
        let mut events = Vec::new();
        for node in fresh.nodes().values() {
            match graph.get_node(&node.node_id) {
                Some(existing) => {
                    if existing.last_observed != node.last_observed {
                        graph.upsert_node(node.clone());
                    }
                }
                None => {
                    graph.upsert_node(node.clone());
                    if is_dynamic(&node.node_type) {
                        events.push(DiscoveryEvent {
                            event_type: EventType::DeviceAdded,
                            node_id: node.node_id.clone(),
                            timestamp: self.options.now,
                        });
                    }
                }
            }
        }
        for edge in fresh.edges() {
            if !graph.has_edge(&edge.source_node, &edge.target_node, edge.edge_type) {
                let _ = graph.add_edge(edge);
            }
        }
        let existing_ids: Vec<NodeId> = graph
            .nodes()
            .values()
            .filter(|n| matches!(n.source, ProvenanceSource::Discovered { .. }))
            .map(|n| n.node_id.clone())
            .collect();
        for id in existing_ids {
            if fresh.get_node(&id).is_none() {
                let removed = graph.remove_node(&id).expect("node present");
                if is_dynamic(&removed.node_type) {
                    events.push(DiscoveryEvent {
                        event_type: EventType::DeviceRemoved,
                        node_id: id,
                        timestamp: self.options.now,
                    });
                }
            }
        }
        events.sort_by_key(|e| e.node_id.to_string());
        Ok(events)
    }

    fn read(&self, root: &Path, rel: &str) -> Result<String, DiscoveryError> {
        let path = root.join(rel);
        let source = std::fs::read_to_string(&path).map_err(|e| DiscoveryError::ReadFailed {
            path: path.display().to_string(),
            source: e,
        })?;
        Ok(source)
    }

    fn read_optional(&self, root: &Path, rel: &str) -> Option<String> {
        self.read(root, rel).ok()
    }

    fn list_dir(&self, root: &Path, rel: &str) -> Vec<String> {
        let Ok(entries) = std::fs::read_dir(root.join(rel)) else {
            return Vec::new();
        };
        let mut names: Vec<String> = entries
            .filter_map(|e| e.ok())
            .filter_map(|e| e.file_name().into_string().ok())
            .collect();
        names.sort();
        names
    }

    fn symlink_name(&self, root: &Path, rel: &str) -> Option<String> {
        let path = root.join(rel);
        let link = std::fs::read_link(&path).ok()?;
        link.file_name().map(|n| n.to_string_lossy().into_owned())
    }

    fn add_node(
        &self,
        graph: &mut SystemGraph,
        id: NodeId,
        node_type: NodeType,
        label: String,
        version: Option<String>,
        t: Timestamp,
        expires: Option<Timestamp>,
        attributes: HashMap<String, String>,
    ) {
        let mut node = NodeMetadata::new(
            id.clone(),
            node_type,
            ProvenanceSource::Discovered { via: "sysfs".into() },
            TrustLevel::Provisional,
            t,
        );
        node.label = label;
        node.version = version;
        node.expires_at = expires;
        node.attributes = attributes;
        let _ = graph.add_node(node);
    }

    fn add_depends_on(
        &self,
        graph: &mut SystemGraph,
        from: &NodeId,
        to: &NodeId,
        t: Timestamp,
    ) {
        if graph.get_node(to).is_none() {
            return;
        }
        let _ = graph.add_edge(EdgeMetadata {
            edge_id: EdgeId::new(),
            edge_type: EdgeType::DependsOn,
            source_node: from.clone(),
            target_node: to.clone(),
            provenance: EdgeProvenance::Observed {
                observed_by: PrincipalId::system("discovery"),
                event_type: EventType::DeviceAdded,
            },
            created_at: t,
            last_observed: t,
            expires_at: None,
            attributes: HashMap::new(),
        });
    }

    fn ensure_driver(
        &self,
        graph: &mut SystemGraph,
        name: &str,
        t: Timestamp,
        expires: Option<Timestamp>,
    ) -> NodeId {
        let id = NodeId(format!("driver:{name}"));
        if graph.get_node(&id).is_none() {
            self.add_node(
                graph,
                id.clone(),
                NodeType::Driver,
                format!("kernel driver {name}"),
                None,
                t,
                expires,
                HashMap::new(),
            );
        }
        id
    }

    fn discover_kernel(
        &self,
        root: &Path,
        graph: &mut SystemGraph,
        t: Timestamp,
        expires: Option<Timestamp>,
    ) -> Result<(), DiscoveryError> {
        let Some(raw) = self.read_optional(root, "proc/sys/kernel/osrelease") else {
            return Ok(());
        };
        let version = raw.trim().to_string();
        if version.is_empty() {
            return Ok(());
        }
        let id = NodeId(format!("kernel:linux-{version}"));
        self.add_node(
            graph,
            id,
            NodeType::Kernel,
            format!("linux {version}"),
            Some(version),
            t,
            expires,
            HashMap::new(),
        );
        Ok(())
    }

    fn discover_drivers(
        &self,
        root: &Path,
        graph: &mut SystemGraph,
        t: Timestamp,
        expires: Option<Timestamp>,
    ) -> Result<(), DiscoveryError> {
        let Some(data) = self.read_optional(root, "proc/modules") else {
            return Ok(());
        };
        for line in data.lines() {
            let name = line.split_whitespace().next().unwrap_or_default();
            if !name.is_empty() {
                self.ensure_driver(graph, name, t, expires);
            }
        }
        Ok(())
    }

    fn discover_cpu(
        &self,
        root: &Path,
        graph: &mut SystemGraph,
        t: Timestamp,
        expires: Option<Timestamp>,
    ) -> Result<(), DiscoveryError> {
        let Some(data) = self.read_optional(root, "proc/cpuinfo") else {
            return Ok(());
        };
        let mut vendor = String::new();
        let mut model = String::new();
        let mut count = 0usize;
        for line in data.lines() {
            let Some((key, value)) = line.split_once(':') else {
                continue;
            };
            match (key.trim(), value.trim()) {
                ("processor", _) => count += 1,
                ("vendor_id", v) => vendor = v.to_string(),
                ("model name", v) => model = v.to_string(),
                _ => {}
            }
        }
        for i in 0..count {
            let mut attrs = HashMap::new();
            if !vendor.is_empty() {
                attrs.insert("vendor".into(), vendor.clone());
            }
            if !model.is_empty() {
                attrs.insert("model".into(), model.clone());
            }
            self.add_node(
                graph,
                NodeId(format!("cpu:{i}")),
                NodeType::Cpu,
                if model.is_empty() {
                    format!("cpu {i}")
                } else {
                    format!("cpu {i} ({model})")
                },
                None,
                t,
                expires,
                attrs,
            );
        }
        Ok(())
    }

    fn discover_memory(
        &self,
        root: &Path,
        graph: &mut SystemGraph,
        t: Timestamp,
        expires: Option<Timestamp>,
    ) -> Result<(), DiscoveryError> {
        let Some(data) = self.read_optional(root, "proc/meminfo") else {
            return Ok(());
        };
        let mut mem_total_kb = String::new();
        let mut mem_avail_kb = String::new();
        for line in data.lines() {
            let Some((key, value)) = line.split_once(':') else {
                continue;
            };
            let value = value.trim();
            match key.trim() {
                "MemTotal" => mem_total_kb = value.to_string(),
                "MemAvailable" => mem_avail_kb = value.to_string(),
                _ => {}
            }
        }
        let mut add_memory = |id: &str, label: &str, kb: &str| {
            let mut attrs = HashMap::new();
            if !kb.is_empty() {
                let kb_num = kb.trim_end_matches("kB").trim();
                attrs.insert("size_kb".into(), kb_num.to_string());
            }
            self.add_node(
                graph,
                NodeId(id.into()),
                NodeType::Memory,
                label.into(),
                None,
                t,
                expires,
                attrs,
            );
        };
        if !mem_total_kb.is_empty() {
            add_memory("memory:total", &format!("total memory ({mem_total_kb})"), &mem_total_kb);
        }
        if !mem_avail_kb.is_empty() {
            add_memory(
                "memory:available",
                &format!("available memory ({mem_avail_kb})"),
                &mem_avail_kb,
            );
        }
        Ok(())
    }

    fn discover_network(
        &self,
        root: &Path,
        graph: &mut SystemGraph,
        t: Timestamp,
        expires: Option<Timestamp>,
    ) -> Result<(), DiscoveryError> {
        for iface in self.list_dir(root, "sys/class/net") {
            let mut attrs = HashMap::new();
            if let Some(mac) = self.read_optional(root, &format!("sys/class/net/{iface}/address")) {
                attrs.insert("mac".into(), mac.trim().to_string());
            }
            if let Some(mtu) = self.read_optional(root, &format!("sys/class/net/{iface}/mtu")) {
                attrs.insert("mtu".into(), mtu.trim().to_string());
            }
            let state = self
                .read_optional(root, &format!("sys/class/net/{iface}/operstate"))
                .map(|s| s.trim().to_string())
                .unwrap_or_default();
            if !state.is_empty() {
                attrs.insert("operstate".into(), state.clone());
            }
            let id = NodeId(format!("device:net-{iface}"));
            self.add_node(
                graph,
                id.clone(),
                NodeType::Device,
                format!("network interface {iface}"),
                None,
                t,
                expires,
                attrs,
            );
            if state == "up" {
                graph.update_health(&id, HealthState::Healthy);
            }
        }
        Ok(())
    }

    /// Resolve `sys/class/net/<iface>/device` (a symlink to the PCI/USB
    /// device path) to the slot identifier used for `device:pci-<slot>` or
    /// `device:usb-<id>` nodes. Returns `None` if the link cannot be read.
    fn underlying_device_slot(&self, root: &Path, rel: &str) -> Option<String> {
        let path = root.join(rel);
        let target = std::fs::read_link(&path).ok()?;
        let leaf = target.file_name()?.to_string_lossy().into_owned();
        if leaf.is_empty() {
            None
        } else {
            Some(leaf)
        }
    }

    fn discover_pci(
        &self,
        root: &Path,
        graph: &mut SystemGraph,
        t: Timestamp,
        expires: Option<Timestamp>,
    ) -> Result<(), DiscoveryError> {
        for slot in self.list_dir(root, "sys/bus/pci/devices") {
            let mut parts = slot.splitn(3, ':');
            let domain = parts.next().unwrap_or_default();
            let bus = parts.next().unwrap_or_default();
            let bus_id = NodeId(format!("bus:pci{domain}:{bus}"));
            self.add_node(
                graph,
                bus_id.clone(),
                NodeType::Bus,
                format!("PCI bus {domain}:{bus}"),
                None,
                t,
                expires,
                HashMap::new(),
            );
            let base = format!("sys/bus/pci/devices/{slot}");
            let mut attrs = HashMap::new();
            for (key, file) in [("vendor", "vendor"), ("device", "device"), ("class", "class")] {
                if let Some(v) = self.read_optional(root, &format!("{base}/{file}")) {
                    attrs.insert(key.into(), v.trim().to_string());
                }
            }
            let id = NodeId(format!("device:pci-{slot}"));
            self.add_node(
                graph,
                id.clone(),
                NodeType::Device,
                format!("PCI device {slot}"),
                None,
                t,
                expires,
                attrs,
            );
            self.add_depends_on(graph, &id, &bus_id, t);
            if let Some(driver) = self.symlink_name(root, &format!("{base}/driver")) {
                let driver_id = self.ensure_driver(graph, &driver, t, expires);
                self.add_depends_on(graph, &id, &driver_id, t);
            }
        }
        Ok(())
    }

    fn discover_usb(
        &self,
        root: &Path,
        graph: &mut SystemGraph,
        t: Timestamp,
        expires: Option<Timestamp>,
    ) -> Result<(), DiscoveryError> {
        for id in self.list_dir(root, "sys/bus/usb/devices") {
            if id.contains(':') {
                continue;
            }
            let base = format!("sys/bus/usb/devices/{id}");
            let mut attrs = HashMap::new();
            for (key, file) in [
                ("vendor", "idVendor"),
                ("product", "idProduct"),
                ("manufacturer", "manufacturer"),
                ("name", "product"),
            ] {
                if let Some(v) = self.read_optional(root, &format!("{base}/{file}")) {
                    attrs.insert(key.into(), v.trim().to_string());
                }
            }
            let node_id = NodeId(format!("device:usb-{id}"));
            self.add_node(
                graph,
                node_id.clone(),
                NodeType::Device,
                format!("USB device {id}"),
                None,
                t,
                expires,
                attrs,
            );
            let bus_id = NodeId("bus:usb0".into());
            self.add_node(
                graph,
                bus_id.clone(),
                NodeType::Bus,
                "USB bus".into(),
                None,
                t,
                expires,
                HashMap::new(),
            );
            self.add_depends_on(graph, &node_id, &bus_id, t);
        }
        Ok(())
    }

    /// Probe every PCI/USB device directory for firmware attributes (a
    /// generic set of standard sysfs names; drivers differ in which one they
    /// expose) and the kernel firmware class, creating `firmware:<name>` nodes
    /// and `depends_on` edges from each device to its firmware. The firmware
    /// version is discovered, never assumed: a device with no readable
    /// firmware attribute simply has no firmware node (M6 acceptance #6).
    fn discover_firmware(
        &self,
        root: &Path,
        graph: &mut SystemGraph,
        t: Timestamp,
        expires: Option<Timestamp>,
    ) -> Result<(), DiscoveryError> {
        for slot in self.list_dir(root, "sys/bus/pci/devices") {
            let base = format!("sys/bus/pci/devices/{slot}");
            if let Some(fw) = self.firmware_attribute(root, &base) {
                self.add_firmware(
                    graph,
                    Some(&NodeId(format!("device:pci-{slot}"))),
                    &fw,
                    t,
                    expires,
                );
            }
        }
        for id in self.list_dir(root, "sys/bus/usb/devices") {
            if id.contains(':') {
                continue;
            }
            let base = format!("sys/bus/usb/devices/{id}");
            if let Some(fw) = self.firmware_attribute(root, &base) {
                self.add_firmware(
                    graph,
                    Some(&NodeId(format!("device:usb-{id}"))),
                    &fw,
                    t,
                    expires,
                );
            }
        }
        // The kernel firmware class is transient: entries appear while a
        // firmware request is in flight. Control files are not firmware.
        // Firmware names are paths relative to the firmware search path and
        // may nest in subdirectories (e.g. `nvidia/gsp`), so the walk is
        // recursive.
        for entry in self.firmware_class_entries(root, "sys/class/firmware") {
            self.add_firmware(graph, None, &entry, t, expires);
        }
        Ok(())
    }

    fn firmware_class_entries(&self, root: &Path, rel: &str) -> Vec<String> {
        let mut entries = Vec::new();
        for name in self.list_dir(root, rel) {
            if FIRMWARE_CLASS_CONTROL_FILES.contains(&name.as_str()) {
                continue;
            }
            let nested = format!("{rel}/{name}");
            let children = self.list_dir(root, &nested);
            if children.is_empty() {
                entries.push(name);
            } else {
                for child in children {
                    if !FIRMWARE_CLASS_CONTROL_FILES.contains(&child.as_str()) {
                        entries.push(format!("{name}/{child}"));
                    }
                }
            }
        }
        entries.sort();
        entries
    }

    fn firmware_attribute(&self, root: &Path, base: &str) -> Option<String> {
        for name in FIRMWARE_ATTRIBUTES {
            if let Some(value) = self.read_optional(root, &format!("{base}/{name}")) {
                let value = value.trim();
                if !value.is_empty() {
                    return Some(value.to_string());
                }
            }
        }
        None
    }

    fn add_firmware(
        &self,
        graph: &mut SystemGraph,
        device: Option<&NodeId>,
        name: &str,
        t: Timestamp,
        expires: Option<Timestamp>,
    ) {
        let slug = name.trim().replace(['/', ' '], "-");
        if slug.is_empty() {
            return;
        }
        let id = NodeId(format!("firmware:{slug}"));
        if graph.get_node(&id).is_none() {
            self.add_node(
                graph,
                id.clone(),
                NodeType::Firmware,
                format!("firmware {name}"),
                None,
                t,
                expires,
                HashMap::new(),
            );
        }
        if let Some(device) = device {
            self.add_depends_on(graph, device, &id, t);
        }
    }

    fn discover_block(
        &self,
        root: &Path,
        graph: &mut SystemGraph,
        t: Timestamp,
        expires: Option<Timestamp>,
    ) -> Result<(), DiscoveryError> {
        for name in self.list_dir(root, "sys/class/block") {
            let base = format!("sys/class/block/{name}");
            let mut attrs = HashMap::new();
            if let Some(size) = self.read_optional(root, &format!("{base}/size")) {
                if let Ok(sectors) = size.trim().parse::<u64>() {
                    attrs.insert("size_bytes".into(), format!("{}", sectors.saturating_mul(512)));
                }
            }
            for (key, file) in [("read_only", "ro"), ("removable", "removable")] {
                if let Some(v) = self.read_optional(root, &format!("{base}/{file}")) {
                    attrs.insert(key.into(), v.trim().to_string());
                }
            }
            let id = NodeId(format!("device:{name}"));
            self.add_node(
                graph,
                id.clone(),
                NodeType::Device,
                format!("block device {name}"),
                None,
                t,
                expires,
                attrs,
            );
            if let Some(driver) = self.symlink_name(root, &format!("{base}/device/driver")) {
                let driver_id = self.ensure_driver(graph, &driver, t, expires);
                self.add_depends_on(graph, &id, &driver_id, t);
            }
        }
        Ok(())
    }

    fn discover_filesystems(
        &self,
        root: &Path,
        graph: &mut SystemGraph,
        t: Timestamp,
        expires: Option<Timestamp>,
    ) -> Result<(), DiscoveryError> {
        let Some(data) = self.read_optional(root, "proc/mounts") else {
            return Ok(());
        };
        let pseudo: &[&str] = &[
            "proc", "sysfs", "devpts", "tmpfs", "devtmpfs", "cgroup", "cgroup2", "pstore",
            "securityfs", "debugfs", "mqueue", "hugetlbfs", "configfs", "fusectl", "bpf",
            "tracefs", "ramfs", "overlay", "autofs", "binfmt_misc", "selinuxfs",
        ];
        for line in data.lines() {
            let mut fields = line.split_whitespace();
            let Some(device) = fields.next() else { continue };
            let Some(mountpoint) = fields.next() else { continue };
            let Some(fstype) = fields.next() else { continue };
            if pseudo.contains(&fstype) {
                continue;
            }
            let slug = mountpoint
                .replace('/', "-")
                .trim_matches('-')
                .to_string();
            let slug = if slug.is_empty() { "root".to_string() } else { slug };
            let mut attrs = HashMap::new();
            if !device.is_empty() {
                attrs.insert("device".into(), device.to_string());
            }
            self.add_node(
                graph,
                NodeId(format!("fs:{fstype}-{slug}")),
                NodeType::Filesystem,
                format!("{fstype} mounted at {mountpoint}"),
                None,
                t,
                expires,
                attrs,
            );
        }
        Ok(())
    }

    fn discover_sensors(
        &self,
        root: &Path,
        graph: &mut SystemGraph,
        t: Timestamp,
        expires: Option<Timestamp>,
    ) -> Result<(), DiscoveryError> {
        for hwmon in self.list_dir(root, "sys/class/hwmon") {
            let base = format!("sys/class/hwmon/{hwmon}");
            let name = self
                .read_optional(root, &format!("{base}/name"))
                .map(|s| s.trim().to_string())
                .unwrap_or_else(|| hwmon.clone());
            let mut inputs: Vec<(String, String, Option<String>)> = Vec::new();
            for file in self.list_dir(root, &base) {
                let Some(stripped) = file.strip_suffix("_input") else {
                    continue;
                };
                if let Some(value) = self.read_optional(root, &format!("{base}/{file}")) {
                    let label = self
                        .read_optional(root, &format!("{base}/{stripped}_label"))
                        .map(|s| s.trim().to_string());
                    inputs.push((stripped.to_string(), value.trim().to_string(), label));
                }
            }
            inputs.sort_by(|a, b| a.0.cmp(&b.0));
            for (kind, value, label) in inputs {
                let mut attrs = HashMap::new();
                attrs.insert("chip".into(), name.clone());
                let unit = if kind.starts_with("temp") {
                    "millidegree_c"
                } else if kind.starts_with("fan") {
                    "rpm"
                } else if kind.starts_with("in") {
                    "millivolt"
                } else {
                    "raw"
                };
                attrs.insert("unit".into(), unit.into());
                attrs.insert("value".into(), value.clone());
                if let Some(label) = label {
                    attrs.insert("label".into(), label);
                }
                let id = NodeId(format!("sensor:{hwmon}-{kind}"));
                self.add_node(
                    graph,
                    id,
                    NodeType::Sensor,
                    format!("{name} {kind}"),
                    None,
                    t,
                    expires,
                    attrs,
                );
            }
        }
        Ok(())
    }

    /// Discover running processes from `/proc`. Each numeric directory is a
    /// process; `comm` gives the name, `stat` the state plus the utime/stime
    /// tick counters, `status` the memory usage, and `cmdline` the full
    /// command line. The state char drives the node health. The processes
    /// specialist owns the `process:*` nodes.
    fn discover_processes(
        &self,
        root: &Path,
        graph: &mut SystemGraph,
        t: Timestamp,
        expires: Option<Timestamp>,
    ) -> Result<(), DiscoveryError> {
        for entry in self.list_dir(root, "proc") {
            let Ok(pid) = entry.parse::<u32>() else {
                continue;
            };
            let base = format!("proc/{pid}");
            let Some(comm) = self.read_optional(root, &format!("{base}/comm")) else {
                continue;
            };
            let comm = comm.trim().to_string();
            if comm.is_empty() {
                continue;
            }
            let mut attrs = HashMap::new();
            attrs.insert("pid".into(), pid.to_string());
            attrs.insert("comm".into(), comm.clone());
            let mut stat_state: Option<char> = None;
            if let Some(stat) = self.read_optional(root, &format!("{base}/stat")) {
                // stat field 3 (after pid and comm in parens) is the state;
                // utime/stime are the 12th and 13th fields after the paren.
                if let Some(open) = stat.rfind(')') {
                    let rest: Vec<&str> = stat[open + 1..].split_whitespace().collect();
                    if let Some(state) = rest.first().filter(|s| !s.is_empty()) {
                        attrs.insert("state".into(), (*state).to_string());
                        stat_state = state.chars().next();
                    }
                    if rest.len() >= 13 {
                        attrs.insert("cpu_utime_ticks".into(), rest[11].to_string());
                        attrs.insert("cpu_stime_ticks".into(), rest[12].to_string());
                    }
                }
            }
            if let Some(cmdline) = self.read_optional(root, &format!("{base}/cmdline")) {
                let joined = cmdline
                    .split('\0')
                    .filter(|part| !part.is_empty())
                    .collect::<Vec<_>>()
                    .join(" ");
                if !joined.is_empty() {
                    let mut truncated = joined;
                    truncated.truncate(256);
                    attrs.insert("cmdline".into(), truncated);
                }
            }
            if let Some(status) = self.read_optional(root, &format!("{base}/status")) {
                for line in status.lines() {
                    let Some((key, value)) = line.split_once(':') else {
                        continue;
                    };
                    let value = value.trim();
                    match key.trim() {
                        "VmRSS" => {
                            let kb = value.trim_end_matches("kB").trim();
                            attrs.insert("rss_kb".into(), kb.to_string());
                        }
                        "State" => {
                            if !attrs.contains_key("state") {
                                attrs.insert("state".into(), value.to_string());
                            }
                        }
                        _ => {}
                    }
                }
            }
            let id = NodeId(format!("process:{pid}"));
            self.add_node(
                graph,
                id.clone(),
                NodeType::Process,
                format!("process {pid} ({comm})"),
                None,
                t,
                expires,
                attrs,
            );
            let Some(state) = stat_state else {
                continue;
            };
            let Some(mut node) = graph.get_node(&id) else {
                continue;
            };
            node.health = process_health(state);
            graph.upsert_node(node);
        }
        Ok(())
    }
}

/// Map a `/proc` process state char to a node health: running, sleeping, and
/// idle are healthy; disk-wait, zombie, and stopped are degraded; anything
/// else stays unknown because discovery cannot confirm it.
fn process_health(state: char) -> HealthState {
    match state {
        'R' | 'S' | 'I' => HealthState::Healthy,
        'D' | 'Z' | 'T' | 't' | 'X' | 'x' => HealthState::Degraded,
        _ => HealthState::Unknown,
    }
}

pub struct ServiceDiscovery {
    systemctl: Vec<String>,
}

impl Default for ServiceDiscovery {
    fn default() -> Self {
        Self::new()
    }
}

impl ServiceDiscovery {
    pub fn new() -> Self {
        Self {
            systemctl: vec![
                "systemctl".into(),
                "--no-legend".into(),
                "--no-pager".into(),
                "--plain".into(),
                "list-units".into(),
                "--type=service".into(),
            ],
        }
    }

    pub fn with_command(command: Vec<String>) -> Self {
        Self { systemctl: command }
    }

    pub fn scan(&self) -> Result<Vec<DiscoveredService>, DiscoveryError> {
        let output = std::process::Command::new(&self.systemctl[0])
            .args(&self.systemctl[1..])
            .output()
            .map_err(|e| DiscoveryError::ReadFailed {
                path: self.systemctl[0].clone(),
                source: e,
            })?;
        if !output.status.success() {
            return Err(DiscoveryError::ReadFailed {
                path: self.systemctl[0].clone(),
                source: std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("systemctl exited with {}", output.status),
                ),
            });
        }
        let text = String::from_utf8_lossy(&output.stdout).into_owned();
        Ok(parse_systemctl_units(&text))
    }

    pub fn populate(&self, graph: &mut SystemGraph, t: Timestamp) -> Result<usize, DiscoveryError> {
        let services = self.scan()?;
        let count = services.len();
        for service in services {
            let mut node = NodeMetadata::new(
                NodeId(format!("service:{}", service.name)),
                NodeType::Service,
                ProvenanceSource::Discovered { via: "systemctl".into() },
                TrustLevel::Provisional,
                t,
            );
            node.label = format!("{} ({})", service.description, service.state);
            let mut attrs = HashMap::new();
            attrs.insert("state".into(), service.state.clone());
            attrs.insert("description".into(), service.description.clone());
            node.attributes = attrs;
            if service.state == "active" {
                node.health = HealthState::Healthy;
            } else {
                node.health = HealthState::Degraded;
            }
            graph.upsert_node(node);
        }
        Ok(count)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiscoveredService {
    pub name: String,
    pub state: String,
    pub description: String,
}

pub fn parse_systemctl_units(output: &str) -> Vec<DiscoveredService> {
    output
        .lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            let name = fields.next()?;
            fields.next()?;
            let state = fields.next()?.to_string();
            fields.next()?;
            if !name.ends_with(".service") {
                return None;
            }
            let description = fields.collect::<Vec<&str>>().join(" ");
            Some(DiscoveredService {
                name: name.trim_end_matches(".service").to_string(),
                state,
                description,
            })
        })
        .collect()
}

pub fn print_hardware_report(graph: &SystemGraph) {
    println!("== discovered hardware ==");
    let mut total = 0usize;
    for node_type in [
        NodeType::Kernel,
        NodeType::Cpu,
        NodeType::Memory,
        NodeType::Bus,
        NodeType::Device,
        NodeType::Driver,
        NodeType::Filesystem,
        NodeType::Service,
        NodeType::Sensor,
    ] {
        let nodes = graph.get_nodes_by_type(node_type);
        if nodes.is_empty() {
            continue;
        }
        println!("{:?}: {}", node_type, nodes.len());
        total += nodes.len();
        for node in nodes {
            let stale = node.is_stale(now());
            let health = if stale {
                "STALE".to_string()
            } else {
                format!("{:?}", node.health)
            };
            let mut extra = String::new();
            if let Some(version) = &node.version {
                extra = format!(" v{version}");
            }
            println!("  {} ({}){extra} [{health}]", node.node_id, node.label);
        }
    }
    println!("total nodes: {total}");
    println!("== end hardware report ==");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write(path: &Path, content: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, content).unwrap();
    }

    fn mock_root() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_path_buf();
        write(
            &root.join("proc/sys/kernel/osrelease"),
            "6.8.0-45-generic\n",
        );
        write(
            &root.join("proc/cpuinfo"),
            "processor\t: 0\nvendor_id\t: GenuineIntel\nmodel name\t: Intel Core i5-1235U\n\nprocessor\t: 1\nvendor_id\t: GenuineIntel\nmodel name\t: Intel Core i5-1235U\n",
        );
        write(
            &root.join("proc/meminfo"),
            "MemTotal:       16384000 kB\nMemAvailable:     8123456 kB\n",
        );
        write(
            &root.join("proc/modules"),
            "iwlwifi 501760 1 - Live 0x0000000000000000\nnvme 122880 2 - Live 0x0000000000000000\n",
        );
        write(
            &root.join("proc/mounts"),
            "/dev/nvme0n1p2 / ext4 rw,relatime 0 0\n/dev/nvme0n1p1 /boot vfat rw 0 0\n",
        );
        write(&root.join("sys/class/net/wlan0/address"), "aa:bb:cc:dd:ee:ff\n");
        write(&root.join("sys/class/net/wlan0/mtu"), "1500\n");
        write(&root.join("sys/class/net/wlan0/operstate"), "up\n");
        // wlan0 is backed by the PCI wireless device 0000:00:14.3.
        std::os::unix::fs::symlink(
            "../../../devices/pci0000:00/0000:00:1c.0/0000:00:14.3",
            root.join("sys/class/net/wlan0/device"),
        )
        .unwrap();
        write(&root.join("sys/class/block/nvme0/size"), "1000215216\n");
        write(&root.join("sys/class/block/nvme0/ro"), "0\n");
        write(&root.join("sys/class/block/nvme0/removable"), "0\n");
        fs::create_dir_all(root.join("sys/devices/pci-drivers/nvme")).unwrap();
        fs::create_dir_all(root.join("sys/class/block/nvme0/device")).unwrap();
        std::os::unix::fs::symlink(
            "../../../devices/pci-drivers/nvme",
            root.join("sys/class/block/nvme0/device/driver"),
        )
        .unwrap();
        write(&root.join("sys/bus/pci/devices/0000:00:14.3/vendor"), "0x8086\n");
        write(&root.join("sys/bus/pci/devices/0000:00:14.3/device"), "0x51f0\n");
        write(&root.join("sys/bus/pci/devices/0000:00:14.3/class"), "0x028000\n");
        fs::create_dir_all(root.join("sys/bus/pci/drivers/iwlwifi")).unwrap();
        std::os::unix::fs::symlink(
            "../../drivers/iwlwifi",
            root.join("sys/bus/pci/devices/0000:00:14.3/driver"),
        )
        .unwrap();
        write(&root.join("sys/bus/usb/devices/1-1/idVendor"), "0x8087\n");
        write(&root.join("sys/bus/usb/devices/1-1/idProduct"), "0x0026\n");
        write(&root.join("sys/bus/usb/devices/1-1/product"), "Wireless Adapter\n");
        (dir, root)
    }

    fn discovery(root: PathBuf) -> SysfsDiscovery {
        SysfsDiscovery::with_options(DiscoveryOptions {
            root,
            now: 1_000,
            ttl: 60,
        })
    }

    #[test]
    fn scan_populates_graph_from_sysfs() {
        let (_dir, root) = mock_root();
        let graph = discovery(root).scan().unwrap();

        assert!(graph.get_node(&NodeId("kernel:linux-6.8.0-45-generic".into())).is_some());
        assert!(graph.get_node(&NodeId("cpu:0".into())).is_some());
        assert!(graph.get_node(&NodeId("cpu:1".into())).is_some());
        assert!(graph.get_node(&NodeId("memory:total".into())).is_some());
        assert!(graph.get_node(&NodeId("device:net-wlan0".into())).is_some());
        assert!(graph.get_node(&NodeId("device:pci-0000:00:14.3".into())).is_some());
        assert!(graph.get_node(&NodeId("bus:pci0000:00".into())).is_some());
        assert!(graph.get_node(&NodeId("device:usb-1-1".into())).is_some());
        assert!(graph.get_node(&NodeId("device:nvme0".into())).is_some());
        assert!(graph.get_node(&NodeId("driver:iwlwifi".into())).is_some());
        assert!(graph.get_node(&NodeId("driver:nvme".into())).is_some());
        assert!(graph.get_node(&NodeId("fs:ext4-root".into())).is_some());
        assert!(graph.get_node(&NodeId("fs:vfat-boot".into())).is_some());
    }

    #[test]
    fn discovery_adds_dependency_edges() {
        let (_dir, root) = mock_root();
        let graph = discovery(root).scan().unwrap();
        let wifi = NodeId("device:pci-0000:00:14.3".into());
        let deps: Vec<String> = graph
            .get_dependencies(&wifi)
            .into_iter()
            .map(|n| n.node_id.to_string())
            .collect();
        assert!(deps.contains(&"bus:pci0000:00".to_string()), "deps: {deps:?}");
        assert!(deps.contains(&"driver:iwlwifi".to_string()), "deps: {deps:?}");

        let nvme = NodeId("device:nvme0".into());
        let nvme_deps: Vec<String> = graph
            .get_dependencies(&nvme)
            .into_iter()
            .map(|n| n.node_id.to_string())
            .collect();
        assert!(nvme_deps.contains(&"driver:nvme".to_string()), "deps: {nvme_deps:?}");
    }

    #[test]
    fn network_interface_links_to_underlying_pci_device() {
        let (_dir, root) = mock_root();
        let graph = discovery(root).scan().unwrap();
        let iface = NodeId("device:net-wlan0".into());
        let deps: Vec<String> = graph
            .get_dependencies(&iface)
            .into_iter()
            .map(|n| n.node_id.to_string())
            .collect();
        assert!(
            deps.contains(&"device:pci-0000:00:14.3".to_string()),
            "deps: {deps:?}"
        );
    }

    #[test]
    fn discovered_nodes_go_stale_after_ttl() {
        let (_dir, root) = mock_root();
        let graph = discovery(root).scan().unwrap();
        let wifi = NodeId("device:pci-0000:00:14.3".into());
        let node = graph.get_node(&wifi).unwrap();
        assert!(!node.is_stale(1_059));
        assert!(node.is_stale(1_061));
    }

    #[test]
    fn network_health_reflects_operstate() {
        let (_dir, root) = mock_root();
        let graph = discovery(root).scan().unwrap();
        let wlan = NodeId("device:net-wlan0".into());
        assert_eq!(graph.get_health(&wlan), HealthState::Healthy);
    }

    #[test]
    fn missing_root_is_reported() {
        let dir = tempfile::tempdir().unwrap();
        let err = discovery(dir.path().to_path_buf()).scan().unwrap_err();
        assert!(matches!(err, DiscoveryError::RootMissing(_)));
    }

    #[test]
    fn empty_tree_produces_empty_graph() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("proc");
        fs::create_dir_all(&root).unwrap();
        let graph = discovery(dir.path().to_path_buf()).scan().unwrap();
        assert!(graph.nodes().is_empty());
    }

    #[test]
    fn sensors_are_discovered_from_hwmon() {
        let (_dir, root) = mock_root();
        write(&root.join("sys/class/hwmon/hwmon0/name"), "coretemp\n");
        write(&root.join("sys/class/hwmon/hwmon0/temp1_input"), "52000\n");
        write(&root.join("sys/class/hwmon/hwmon0/temp1_label"), "Package id 0\n");
        write(&root.join("sys/class/hwmon/hwmon1/name"), "nct6798\n");
        write(&root.join("sys/class/hwmon/hwmon1/fan1_input"), "1200\n");
        let graph = discovery(root).scan().unwrap();
        let temp = graph.get_node(&NodeId("sensor:hwmon0-temp1".into())).unwrap();
        assert_eq!(temp.node_type, NodeType::Sensor);
        assert_eq!(temp.attributes.get("value").unwrap(), "52000");
        assert_eq!(temp.attributes.get("unit").unwrap(), "millidegree_c");
        assert_eq!(temp.attributes.get("label").unwrap(), "Package id 0");
        let fan = graph.get_node(&NodeId("sensor:hwmon1-fan1".into())).unwrap();
        assert_eq!(fan.attributes.get("unit").unwrap(), "rpm");
    }

    #[test]
    fn device_firmware_attributes_create_nodes_and_edges() {
        let (_dir, root) = mock_root();
        // The mock wifi PCI device exposes a firmware version attribute; the
        // USB device exposes a different (fw_version) attribute name. Both
        // must be discovered without any per-driver knowledge.
        write(
            &root.join("sys/bus/pci/devices/0000:00:14.3/firmware_version"),
            "iwlwifi-46\n",
        );
        write(
            &root.join("sys/bus/usb/devices/1-1/fw_version"),
            "2.0.1\n",
        );
        let graph = discovery(root).scan().unwrap();

        let fw = graph
            .get_node(&NodeId("firmware:iwlwifi-46".into()))
            .expect("wifi firmware node present");
        assert_eq!(fw.node_type, NodeType::Firmware);
        let wifi_deps: Vec<String> = graph
            .get_dependencies(&NodeId("device:pci-0000:00:14.3".into()))
            .into_iter()
            .map(|n| n.node_id.to_string())
            .collect();
        assert!(
            wifi_deps.contains(&"firmware:iwlwifi-46".to_string()),
            "wifi deps: {wifi_deps:?}"
        );

        assert!(graph.get_node(&NodeId("firmware:2.0.1".into())).is_some());
        let usb_deps: Vec<String> = graph
            .get_dependencies(&NodeId("device:usb-1-1".into()))
            .into_iter()
            .map(|n| n.node_id.to_string())
            .collect();
        assert!(
            usb_deps.contains(&"firmware:2.0.1".to_string()),
            "usb deps: {usb_deps:?}"
        );
    }

    #[test]
    fn firmware_class_entries_create_nodes_but_not_control_files() {
        let (_dir, root) = mock_root();
        fs::create_dir_all(root.join("sys/class/firmware/iwlwifi-ty-a0-gf-a0-83.ucode")).unwrap();
        fs::create_dir_all(root.join("sys/class/firmware/nvidia/gsp")).unwrap();
        write(&root.join("sys/class/firmware/timeout"), "60\n");
        let graph = discovery(root).scan().unwrap();

        assert!(graph
            .get_node(&NodeId("firmware:iwlwifi-ty-a0-gf-a0-83.ucode".into()))
            .is_some());
        // Slashes in the firmware name are sanitized into the node id.
        assert!(graph.get_node(&NodeId("firmware:nvidia-gsp".into())).is_some());
        assert!(
            graph.get_node(&NodeId("firmware:timeout".into())).is_none(),
            "timeout is a control file, not firmware"
        );
    }

    #[test]
    fn devices_without_firmware_attributes_get_no_firmware_node() {
        let (_dir, root) = mock_root();
        let graph = discovery(root).scan().unwrap();
        assert_eq!(
            graph
                .nodes()
                .values()
                .filter(|n| n.node_type == NodeType::Firmware)
                .count(),
            0
        );
    }

    #[test]
    fn process_nodes_parse_ticks_cmdline_and_health() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_path_buf();
        write(&root.join("proc/123/comm"), "aios\n");
        write(
            &root.join("proc/123/stat"),
            "123 (aios worker) S 1 2 3 4 5 6 7 8 9 10 111 222 333 444\n",
        );
        write(
            &root.join("proc/123/status"),
            "Name:\taios\nState:\tS (sleeping)\nVmRSS:\t    123456 kB\n",
        );
        write(&root.join("proc/123/cmdline"), "aios\0serve\0--debug\0");
        // A zombie process must report degraded health.
        write(&root.join("proc/9/comm"), "kworker\n");
        write(
            &root.join("proc/9/stat"),
            "9 (kworker) Z 1 2 3 4 5 6 7 8 9 10 0 0 0 0\n",
        );

        let graph = discovery(root).scan().unwrap();

        let aios = graph.get_node(&NodeId("process:123".into())).unwrap();
        assert_eq!(aios.attributes.get("cpu_utime_ticks").unwrap(), "111");
        assert_eq!(aios.attributes.get("cpu_stime_ticks").unwrap(), "222");
        assert_eq!(
            aios.attributes.get("cmdline").unwrap(),
            "aios serve --debug"
        );
        assert_eq!(aios.attributes.get("rss_kb").unwrap(), "123456");
        assert_eq!(aios.attributes.get("state").unwrap(), "S");
        assert_eq!(aios.health, HealthState::Healthy);

        let zombie = graph.get_node(&NodeId("process:9".into())).unwrap();
        assert_eq!(zombie.health, HealthState::Degraded);
    }

    #[test]
    fn reconcile_emits_add_and_remove_events() {
        let (_dir, root) = mock_root();
        let d = discovery(root.clone());
        let mut graph = d.scan().unwrap();
        assert!(graph.get_node(&NodeId("device:pci-0000:00:14.3".into())).is_some());

        fs::remove_dir_all(root.join("sys/bus/pci/devices/0000:00:14.3")).unwrap();
        write(&root.join("sys/class/net/eth1/operstate"), "down\n");
        write(&root.join("sys/class/net/eth1/address"), "00:11:22:33:44:55\n");

        let events = d.reconcile(&mut graph).unwrap();
        assert!(
            events
                .iter()
                .any(|e| e.event_type == EventType::DeviceRemoved
                    && e.node_id == NodeId("device:pci-0000:00:14.3".into()))
        );
        assert!(
            events
                .iter()
                .any(|e| e.event_type == EventType::DeviceAdded
                    && e.node_id == NodeId("device:net-eth1".into()))
        );
        assert!(graph.get_node(&NodeId("device:pci-0000:00:14.3".into())).is_none());
        assert!(graph.get_node(&NodeId("device:net-eth1".into())).is_some());
    }

    #[test]
    fn reconcile_removes_dangling_edges() {
        let (_dir, root) = mock_root();
        let d = discovery(root.clone());
        let mut graph = d.scan().unwrap();
        let wifi = NodeId("device:pci-0000:00:14.3".into());
        assert!(!graph.get_dependencies(&wifi).is_empty());

        fs::remove_dir_all(root.join("sys/bus/pci/devices/0000:00:14.3")).unwrap();
        d.reconcile(&mut graph).unwrap();
        assert!(graph.get_dependencies(&wifi).is_empty());
    }

    #[test]
    fn systemctl_output_parses_into_services() {
        let out = "networkd-dispatcher.service loaded active running Dispatches libcups\n\
                   cups.service loaded active running CUPS Scheduler\n\
                   ssh.service loaded inactive dead OpenBSD Secure Shell server\n\
                   snapd.service loaded active running Snap Daemon";
        let services = parse_systemctl_units(out);
        assert!(services.iter().any(|s| s.name == "networkd-dispatcher" && s.state == "active"));
        assert!(services.iter().any(|s| s.name == "cups" && s.description == "CUPS Scheduler"));
    }

    #[test]
    fn service_populate_marks_health_by_state() {
        let (_dir, root) = mock_root();
        let mut graph = discovery(root).scan().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("fake-systemctl");
        write(
            &script,
            "#!/bin/sh\nprintf 'networkd-dispatcher.service loaded active running Dispatches libcups\\nssh.service loaded inactive dead OpenBSD SSH\\n'",
        );
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        let services = ServiceDiscovery::with_command(vec![script.to_string_lossy().into_owned()]);
        let count = services.populate(&mut graph, 1_000).unwrap();
        assert_eq!(count, 2);
        let netd = graph.get_node(&NodeId("service:networkd-dispatcher".into())).unwrap();
        assert_eq!(netd.health, HealthState::Healthy);
        assert_eq!(netd.attributes.get("state").unwrap(), "active");
        let ssh = graph.get_node(&NodeId("service:ssh".into())).unwrap();
        assert_eq!(ssh.health, HealthState::Degraded);
    }
}
