use anyhow::Result;
use evdev::{AbsoluteAxisCode, EventType, InputEvent, KeyCode, raw_stream::RawDevice};
use padsmith_core::Input;
use padsmith_protocol::{DeviceInfo, NodeInfo};
use std::{
    collections::BTreeMap,
    fs,
    os::fd::{AsFd, AsRawFd, OwnedFd},
    path::PathBuf,
};

fn attr(device: &udev::Device, name: &str) -> String {
    device
        .attribute_value(name)
        .map(|v| v.to_string_lossy().into_owned())
        .unwrap_or_default()
}
fn prop(device: &udev::Device, name: &str) -> String {
    device
        .property_value(name)
        .map(|v| v.to_string_lossy().into_owned())
        .unwrap_or_default()
}
fn ancestor(device: &udev::Device) -> udev::Device {
    let mut current = device.clone();
    let mut usb = None;
    while let Some(parent) = current.parent() {
        if parent.subsystem().is_some_and(|s| s == "hid") {
            return parent;
        }
        if parent.devtype().is_some_and(|s| s == "usb_interface") {
            return parent;
        }
        if parent.devtype().is_some_and(|s| s == "usb_device") {
            usb = Some(parent.clone());
        }
        current = parent;
    }
    usb.unwrap_or_else(|| device.parent().unwrap_or_else(|| device.clone()))
}
fn is_ours(device: &udev::Device) -> bool {
    // Match both the kernel virtual topology and our exact output names. Never
    // exclude by Microsoft VID/PID: that would exclude real Xbox controllers.
    if device
        .syspath()
        .to_string_lossy()
        .contains("/devices/virtual/input/")
    {
        let name = device
            .parent()
            .map(|p| attr(&p, "name"))
            .unwrap_or_default();
        if name.starts_with("Padsmith Xbox 360")
            || ["Padsmith Keyboard", "Padsmith Mouse"].contains(&name.as_str())
        {
            return true;
        }
    }
    let mut d = Some(device.clone());
    while let Some(node) = d {
        if attr(&node, "phys").starts_with("padsmith/output/") {
            return true;
        }
        d = node.parent();
    }
    false
}
fn connection(device: &udev::Device, serial: &str) -> String {
    let mut node = Some(device.clone());
    while let Some(parent) = node {
        if parent
            .devtype()
            .is_some_and(|d| d == "usb_interface" || d == "usb_device")
        {
            return parent.syspath().to_string_lossy().into_owned();
        }
        node = parent.parent();
    }
    if !serial.is_empty() {
        format!("uniq:{serial}")
    } else {
        ancestor(device).syspath().to_string_lossy().into_owned()
    }
}

/// Enumerate every sibling before deciding whether the physical group is a gamepad.
/// Sensors and touchpads don't necessarily carry ID_INPUT_JOYSTICK.
pub fn discover() -> Result<Vec<DeviceInfo>> {
    let mut groups: BTreeMap<String, (DeviceInfo, bool)> = BTreeMap::new();
    let mut enumerator = udev::Enumerator::new()?;
    enumerator.match_subsystem("input")?;
    enumerator.match_sysname("event*")?;
    for device in enumerator.scan_devices()? {
        if is_ours(&device) {
            continue;
        }
        let Some(node) = device.devnode() else {
            continue;
        };
        let parent = device.parent().unwrap_or_else(|| device.clone());
        let root = ancestor(&device);
        let root_path = root.syspath().to_string_lossy().into_owned();
        let serial = attr(&parent, "uniq");
        let connection = connection(&device, &serial);
        let name = attr(&parent, "name");
        let joystick = prop(&device, "ID_INPUT_JOYSTICK") == "1";
        let fallback = RawDevice::open(node).ok().is_some_and(|d| {
            d.supported_keys()
                .is_some_and(|k| k.contains(KeyCode::BTN_SOUTH))
                && d.supported_absolute_axes()
                    .is_some_and(|a| a.contains(AbsoluteAxisCode::ABS_X))
        });
        let primary = joystick || fallback;
        let readable = RawDevice::open(node).ok();
        let keys = readable
            .as_ref()
            .and_then(|d| d.supported_keys())
            .map(|k| k.iter().map(|c| crate::key_name(c.0)).collect())
            .unwrap_or_default();
        let axes = readable
            .as_ref()
            .and_then(|d| d.get_abs_state().ok())
            .map(|a| {
                a.iter()
                    .enumerate()
                    .filter(|(_, a)| a.maximum > a.minimum)
                    .map(|(i, a)| {
                        (
                            crate::axis_name(i as u16),
                            padsmith_protocol::AxisInfo {
                                minimum: a.minimum,
                                maximum: a.maximum,
                            },
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        let role = if primary {
            "gamepad"
        } else if prop(&device, "ID_INPUT_ACCELEROMETER") == "1" || name.contains("Motion Sensors")
        {
            "motion"
        } else if prop(&device, "ID_INPUT_TOUCHPAD") == "1" || name.contains("Touchpad") {
            "touchpad"
        } else {
            "aux"
        };
        // Stable serial when exposed; topology is an explicit fallback, not a serial.
        let vendor = attr(&parent, "id/vendor");
        let product = attr(&parent, "id/product");
        let id = if serial.is_empty() {
            format!("{vendor}:{product}@{connection}")
        } else {
            format!("{vendor}:{product}:{serial}")
        };
        let entry = groups.entry(root_path.clone()).or_insert_with(|| {
            (
                DeviceInfo {
                    id,
                    name: name.clone(),
                    vendor,
                    product,
                    driver: root
                        .driver()
                        .map(|s| s.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                    serial,
                    physical_path: root_path.clone(),
                    connection,
                    ..Default::default()
                },
                false,
            )
        });
        if primary {
            entry.0.name = name.clone();
        }
        entry.1 |= primary;
        entry.0.nodes.push(NodeInfo {
            path: node.to_string_lossy().into_owned(),
            role: role.into(),
            name,
            primary,
            keys,
            axes,
            vendor: u16::from_str_radix(&attr(&parent, "id/vendor"), 16).unwrap_or(0),
            product: u16::from_str_radix(&attr(&parent, "id/product"), 16).unwrap_or(0),
            serial: attr(&parent, "uniq"),
        });
    }
    let mut raw = udev::Enumerator::new()?;
    raw.match_subsystem("hidraw")?;
    for device in raw.scan_devices()? {
        let root = ancestor(&device).syspath().to_string_lossy().into_owned();
        if let Some((info, _)) = groups.get_mut(&root) {
            if let Some(node) = device.devnode() {
                info.hidraw.push(node.to_string_lossy().into_owned());
            }
            info.hidraw_hidden |= prop(&device, "PADSMITH_HIDRAW_HIDDEN") == "1";
        }
    }
    let mut supply = udev::Enumerator::new()?;
    supply.match_subsystem("power_supply")?;
    for device in supply.scan_devices()? {
        let root = ancestor(&device).syspath().to_string_lossy().into_owned();
        if let Some((info, _)) = groups.get_mut(&root) {
            info.battery = attr(&device, "capacity")
                .parse::<u8>()
                .ok()
                .filter(|v| *v <= 100);
        }
    }
    let mut found: Vec<DeviceInfo> = groups
        .into_values()
        .filter_map(|(mut info, gamepad)| {
            if !gamepad {
                return None;
            }
            info.nodes.sort_by(|a, b| a.path.cmp(&b.path));
            info.hidraw.sort();
            // Disambiguate same-role interfaces without depending on event numbers.
            let mut counts = BTreeMap::<String, usize>::new();
            for node in &mut info.nodes {
                let count = counts.entry(node.role.clone()).or_default();
                *count += 1;
                if *count > 1 {
                    node.role = format!("{}{}", node.role, count);
                }
            }
            Some(info)
        })
        .collect();
    let mut counts = BTreeMap::new();
    for d in &found {
        *counts.entry(d.id.clone()).or_insert(0) += 1;
    }
    for d in &mut found {
        if counts[&d.id] > 1 {
            d.id = format!("{}@{}", d.id, d.connection);
        }
    }
    Ok(found)
}

pub struct CapturedNode {
    pub control: OwnedFd,
    pub info: NodeInfo,
    pub stream: evdev::raw_stream::EventStream,
    pub axes: BTreeMap<u16, (i32, i32)>,
}
pub fn capture(info: &DeviceInfo) -> Result<Vec<CapturedNode>> {
    let mut nodes = Vec::new();
    for node in &info.nodes {
        let mut device = RawDevice::open(&node.path)?;
        anyhow::ensure!(
            device.name().unwrap_or_default() == node.name
                && device.input_id().vendor() == node.vendor
                && device.input_id().product() == node.product
                && (node.serial.is_empty() || device.unique_name() == Some(node.serial.as_str())),
            "Identidade de {} mudou durante o hotplug; captura cancelada",
            node.path
        );
        let axes = device
            .get_abs_state()
            .map(|state| {
                state
                    .iter()
                    .enumerate()
                    .filter(|(_, a)| a.maximum > a.minimum)
                    .map(|(i, a)| (i as u16, (a.minimum, a.maximum)))
                    .collect()
            })
            .unwrap_or_default();
        device.grab()?;
        let control = device.as_fd().try_clone_to_owned()?;
        nodes.push(CapturedNode {
            control,
            info: node.clone(),
            axes,
            stream: device.into_event_stream()?,
        });
    }
    Ok(nodes)
}
pub fn ungrab(control: &OwnedFd) -> std::io::Result<()> {
    // EVIOCGRAB = _IOW('E', 0x90, int), Linux evdev ABI (x86_64/aarch64).
    // SAFETY: descriptor is live; EVIOCGRAB takes an integer, no user pointer.
    if unsafe {
        libc::ioctl(
            control.as_raw_fd(),
            0x40044590 as libc::c_ulong,
            0 as libc::c_int,
        )
    } < 0
    {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}
pub fn normalize(
    node: &NodeInfo,
    axes: &BTreeMap<u16, (i32, i32)>,
    event: InputEvent,
) -> Option<Input> {
    let qualify = |name: String| {
        if node.primary && node.role == "gamepad" {
            name
        } else {
            format!("{}:{name}", node.role)
        }
    };
    match event.event_type() {
        EventType::KEY if event.value() != 2 => Some(Input::Button {
            code: qualify(crate::key_name(event.code())),
            pressed: event.value() != 0,
        }),
        EventType::ABSOLUTE => {
            let (min, max) = axes.get(&event.code()).copied()?;
            if max <= min {
                return None;
            }
            let code = crate::axis_name(event.code());
            let unit =
                ((event.value() as f64 - min as f64) / (max as f64 - min as f64)).clamp(0.0, 1.0);
            let value = if node.primary && (code == "ABS_Z" || code == "ABS_RZ") {
                unit
            } else {
                unit * 2.0 - 1.0
            };
            Some(Input::Axis {
                code: qualify(code),
                value,
            })
        }
        _ => None,
    }
}
pub fn event_label(event: InputEvent) -> String {
    match event.event_type() {
        EventType::KEY => crate::key_name(event.code()),
        EventType::ABSOLUTE => crate::axis_name(event.code()),
        _ => format!("{:?}:{}", event.event_type(), event.code()),
    }
}

/// Blocking netlink listener on a dedicated thread; no periodic discovery loop.
pub fn hotplug(sender: tokio::sync::mpsc::Sender<()>) {
    std::thread::spawn(move || {
        let result = (|| -> Result<()> {
            let monitor = udev::MonitorBuilder::new()?
                .match_subsystem("input")?
                .match_subsystem("hidraw")?
                .match_subsystem("power_supply")?
                .listen()?;
            let mut fd = libc::pollfd {
                fd: monitor.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            };
            loop {
                // SAFETY: fd points to one initialized pollfd and remains owned by monitor.
                let result = unsafe { libc::poll(&mut fd, 1, -1) };
                if result < 0 {
                    if std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted {
                        continue;
                    }
                    return Err(std::io::Error::last_os_error().into());
                }
                for _ in monitor.iter() {
                    if sender.is_closed() {
                        return Ok(());
                    }
                    let _ = sender.try_send(());
                }
            }
        })();
        if let Err(error) = result {
            tracing::error!(%error,"Monitor udev interrompido");
        }
    });
}
pub fn kwin_gamecontroller() -> Option<bool> {
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    let text = fs::read_to_string(config.join("kwinrc")).ok()?;
    let mut section = false;
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            section = line == "[Plugins]";
        }
        if section && let Some(value) = line.strip_prefix("gamecontrollerEnabled=") {
            return value.parse().ok();
        }
    }
    None
}
