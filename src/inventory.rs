use crate::collectors::Snapshot;
use crate::platform::current;
use serde::Serialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize)]
pub struct InventoryResponse {
    pub host: Option<InventoryHost>,
    pub disks: Vec<DiskInventoryView>,
    pub groups: Vec<DiskInventoryGroupView>,
    pub networks: Vec<NetworkInventoryView>,
}

#[derive(Debug, Clone, Serialize)]
pub struct InventoryHost {
    pub hostname: String,
    pub os_name: String,
    pub os_version: String,
    pub kernel_version: String,
    pub architecture: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiskInventoryView {
    pub device: String,
    pub parent: String,
    pub structure: String,
    pub volume_kind: String,
    pub filesystem: String,
    pub filesystem_family: String,
    pub label: String,
    pub uuid: String,
    pub part_uuid: String,
    pub model: String,
    pub serial: String,
    pub transport: String,
    pub reference: String,
    pub scheduler: String,
    pub rotational: bool,
    pub removable: bool,
    pub read_only: bool,
    pub mount_point: String,
    pub mount_points: Vec<String>,
    pub logical_stack: Vec<String>,
    pub slaves: Vec<String>,
    pub holders: Vec<String>,
    pub children: Vec<String>,
    pub protocol: String,
    pub media: String,
    pub total_gb: f64,
    pub used_gb: f64,
    pub free_gb: f64,
    pub usage_pct: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiskInventoryGroupView {
    pub root_device: String,
    pub transport: String,
    pub model: String,
    pub protocol: String,
    pub media: String,
    pub remote: bool,
    pub members: Vec<String>,
    pub mounted_devices: Vec<String>,
    pub filesystems: Vec<String>,
    pub volume_kinds: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct NetworkInventoryView {
    pub interface: String,
    pub topology: String,
    pub family: String,
    pub medium: String,
    pub connections_total: u32,
    pub connections_established: u32,
}

pub fn build_inventory(snapshot: &Snapshot) -> InventoryResponse {
    let disk_metrics_by_device: HashMap<String, _> = snapshot
        .disks
        .iter()
        .map(|disk| (disk.device.clone(), disk))
        .collect();
    let host = snapshot.system.as_ref().map(|system| InventoryHost {
        hostname: system.hostname.clone(),
        os_name: system.os_name.clone(),
        os_version: system.os_version.clone(),
        kernel_version: system.kernel_version.clone(),
        architecture: system.architecture.clone(),
    });

    let raw_inventory = current::read_disk_inventory().unwrap_or_default();
    let disks = if raw_inventory.is_empty() {
        snapshot
            .disks
            .iter()
            .map(|disk| DiskInventoryView {
                device: disk.device.clone(),
                parent: disk.parent.clone(),
                structure: if disk.structure.is_empty() {
                    disk.structure_hint.clone()
                } else {
                    disk.structure.clone()
                },
                volume_kind: disk.volume_kind.clone(),
                filesystem: disk.filesystem.clone(),
                filesystem_family: disk.filesystem_family.clone(),
                label: disk.label.clone(),
                uuid: disk.uuid.clone(),
                part_uuid: disk.part_uuid.clone(),
                model: disk.model.clone(),
                serial: disk.serial.clone(),
                transport: disk.protocol_hint.clone(),
                reference: disk.reference.clone(),
                scheduler: disk.scheduler.clone(),
                rotational: disk.rotational,
                removable: disk.removable,
                read_only: disk.read_only,
                mount_point: disk.mount_point.clone(),
                mount_points: disk.mount_points.clone(),
                logical_stack: disk.logical_stack.clone(),
                slaves: disk.slaves.clone(),
                holders: disk.holders.clone(),
                children: disk.children.clone(),
                protocol: disk.protocol_hint.clone(),
                media: disk.media_hint.clone(),
                total_gb: disk.total_gb,
                used_gb: disk.used_gb,
                free_gb: disk.free_gb,
                usage_pct: disk.usage_pct,
            })
            .collect::<Vec<_>>()
    } else {
        raw_inventory
            .iter()
            .map(|disk| {
                let metrics = disk_metrics_by_device.get(&disk.device).copied();
                DiskInventoryView {
                    device: disk.device.clone(),
                    parent: disk.parent.clone().unwrap_or_default(),
                    structure: if disk.structure.is_empty() {
                        metrics
                            .map(|item| item.structure_hint.clone())
                            .unwrap_or_default()
                    } else {
                        disk.structure.clone()
                    },
                    volume_kind: if disk.volume_kind.is_empty() {
                        metrics
                            .map(|item| item.volume_kind.clone())
                            .unwrap_or_default()
                    } else {
                        disk.volume_kind.clone()
                    },
                    filesystem: disk.filesystem.clone(),
                    filesystem_family: disk.filesystem_family.clone(),
                    label: disk.label.clone(),
                    uuid: disk.uuid.clone(),
                    part_uuid: disk.part_uuid.clone(),
                    model: disk.model.clone(),
                    serial: disk.serial.clone(),
                    transport: disk.transport.clone(),
                    reference: disk.reference.clone(),
                    scheduler: disk.scheduler.clone(),
                    rotational: disk.rotational.unwrap_or(false),
                    removable: disk.removable.unwrap_or(false),
                    read_only: disk.read_only.unwrap_or(false),
                    mount_point: metrics
                        .map(|item| item.mount_point.clone())
                        .filter(|value| !value.is_empty())
                        .or_else(|| disk.mount_points.first().cloned())
                        .unwrap_or_default(),
                    mount_points: if disk.mount_points.is_empty() {
                        metrics
                            .map(|item| item.mount_points.clone())
                            .unwrap_or_default()
                    } else {
                        disk.mount_points.clone()
                    },
                    logical_stack: if disk.logical_stack.is_empty() {
                        metrics
                            .map(|item| item.logical_stack.clone())
                            .unwrap_or_default()
                    } else {
                        disk.logical_stack.clone()
                    },
                    slaves: disk.slaves.clone(),
                    holders: disk.holders.clone(),
                    children: disk.children.clone(),
                    protocol: metrics
                        .map(|item| item.protocol_hint.clone())
                        .unwrap_or_else(|| disk.transport.clone()),
                    media: metrics
                        .map(|item| item.media_hint.clone())
                        .unwrap_or_default(),
                    total_gb: metrics.map(|item| item.total_gb).unwrap_or(0.0),
                    used_gb: metrics.map(|item| item.used_gb).unwrap_or(0.0),
                    free_gb: metrics.map(|item| item.free_gb).unwrap_or(0.0),
                    usage_pct: metrics.map(|item| item.usage_pct).unwrap_or(0.0),
                }
            })
            .collect::<Vec<_>>()
    };

    let networks = snapshot
        .networks
        .iter()
        .map(|net| NetworkInventoryView {
            interface: net.interface.clone(),
            topology: net.topology_hint.clone(),
            family: net.family_hint.clone(),
            medium: net.medium_hint.clone(),
            connections_total: net.connections_total,
            connections_established: net.connections_established,
        })
        .collect::<Vec<_>>();

    let groups = build_disk_groups(&disks);

    InventoryResponse {
        host,
        disks,
        groups,
        networks,
    }
}

fn build_disk_groups(disks: &[DiskInventoryView]) -> Vec<DiskInventoryGroupView> {
    let mut grouped: HashMap<String, Vec<&DiskInventoryView>> = HashMap::new();
    for disk in disks {
        let root = disk
            .logical_stack
            .first()
            .cloned()
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| disk.device.clone());
        grouped.entry(root).or_default().push(disk);
    }

    let mut groups = grouped
        .into_iter()
        .map(|(root_device, members)| {
            let root = members
                .iter()
                .find(|disk| disk.device == root_device)
                .copied()
                .unwrap_or(members[0]);
            DiskInventoryGroupView {
                root_device,
                transport: root.transport.clone(),
                model: root.model.clone(),
                protocol: root.protocol.clone(),
                media: root.media.clone(),
                remote: root.filesystem_family == "remote" || root.structure == "remote-mount",
                members: unique_sorted(
                    members
                        .iter()
                        .map(|disk| disk.device.clone())
                        .collect::<Vec<_>>(),
                ),
                mounted_devices: unique_sorted(
                    members
                        .iter()
                        .filter(|disk| {
                            !disk.mount_point.is_empty() || !disk.mount_points.is_empty()
                        })
                        .map(|disk| disk.device.clone())
                        .collect::<Vec<_>>(),
                ),
                filesystems: unique_sorted(
                    members
                        .iter()
                        .filter(|disk| !disk.filesystem.is_empty())
                        .map(|disk| disk.filesystem.clone())
                        .collect::<Vec<_>>(),
                ),
                volume_kinds: unique_sorted(
                    members
                        .iter()
                        .filter(|disk| !disk.volume_kind.is_empty())
                        .map(|disk| disk.volume_kind.clone())
                        .collect::<Vec<_>>(),
                ),
            }
        })
        .collect::<Vec<_>>();

    groups.sort_by(|a, b| a.root_device.cmp(&b.root_device));
    groups
}

fn unique_sorted(mut values: Vec<String>) -> Vec<String> {
    values.sort();
    values.dedup();
    values
}
