//! `os_info` - Report operating-system, CPU, GPU, hardware, memory, and process
//! information.
//!
//! Implements a read-only introspection tool that describes the host
//! environment ragent is running on: OS family, name, version and kernel,
//! the Linux distribution, CPU architecture/cores/vendor/brand/frequency,
//! graphics adapters (integrated and discrete), graphics-API support
//! (Direct3D, DirectX, Metal, OpenGL, OpenGL ES, Mesa, Vulkan, OptiX, CUDA,
//! ROCm), physical-hardware identity (system/model, chassis, motherboard,
//! firmware, storage devices, network interfaces), physical and swap memory,
//! system uptime and boot time, and the ragent process environment (pid,
//! working directory, shell, username).
//!
//! The module writes no files and makes no network access: platform metrics
//! come from `sysinfo` (cross-platform, pure Rust), GPU
//! adapters and the physical-hardware identity, storage, and network data from
//! read-only `/sys` files on Linux (DMI sysfs, `/sys/block`, `/sys/class/net`),
//! and process values from
//! `std::env`. Graphics-API versions are read from the host's own
//! configuration artefacts (e.g. Vulkan ICD manifests) and, by default, from a
//! fixed, timeout-bounded allowlist of vendor diagnostics (`vulkaninfo`,
//! `glxinfo`, `nvidia-smi`, `rocminfo`, `system_profiler`) so APIs whose version
//! only a live diagnostic can report (OpenGL, OpenGL ES, Mesa) are described
//! too (FR-020, FR-021). It is the single source of truth for the
//! `os_info` tool and the `/osinfo show` slash command (spec `osinfo`): the
//! collector ([`OsInfo::collect`]) and text renderer ([`render_text`]) are
//! `pub` so the TUI renders the identical report without a second copy
//! (FR-016, FR-018).

use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::Result;
use serde::Serialize;
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};

/// Operating-system identity (spec `osinfo` FR-002, FR-003).
///
/// `distribution_id` and `distribution_name` are filled on Linux and carry
/// the `n/a (<family>)` placeholder elsewhere so the schema is stable across
/// platforms.
#[derive(Debug, Clone, Serialize)]
struct OsSection {
    /// OS family, e.g. `linux`, `macos`, `windows`, `freebsd`, or `unknown`.
    family: String,
    /// Human-readable OS name, e.g. `Fedora Linux`.
    name: String,
    /// OS version string, e.g. `41`.
    version: String,
    /// Long OS version string when the platform exposes one.
    long_version: String,
    /// Kernel version, e.g. `6.12.4-200.fc41.x86_64`.
    kernel_version: String,
    /// Hostname of the machine.
    hostname: String,
    /// Linux distribution id (e.g. `fedora`), or `n/a (<family>)` off Linux.
    distribution_id: String,
    /// Linux distribution display name, or `n/a (<family>)` off Linux.
    distribution_name: String,
}

/// CPU architecture and capability report (spec `osinfo` FR-005).
///
/// Metrics the host cannot supply are reported as `unknown` or `0` rather
/// than failing the call (FR-011).
#[derive(Debug, Clone, Serialize)]
struct CpuSection {
    /// CPU architecture, e.g. `x86_64` or `aarch64`.
    architecture: String,
    /// Number of physical (not hyper-threaded) cores, or `0` when unknown.
    physical_cores: usize,
    /// Number of logical cores seen by the OS.
    logical_cores: usize,
    /// CPU vendor id, e.g. `GenuineIntel`, or `unknown`.
    vendor: String,
    /// CPU brand string, e.g. `AMD Ryzen 9 7950X`, or `unknown`.
    brand: String,
    /// Current CPU frequency in MHz, or `0` when unknown.
    frequency_mhz: u64,
}

/// Graphics adapter report (spec `osinfo` FR-019).
///
/// One entry per detected graphics adapter, ordered by DRM card index so the
/// list is stable across calls. Hosts that expose no adapter information (or
/// non-Linux platforms) yield an empty list rather than failing the call.
#[derive(Debug, Clone, Serialize)]
struct GpuSection {
    /// Detected graphics adapters (integrated, discrete, or virtual).
    adapters: Vec<GpuAdapter>,
    /// Detected graphics-API support, in the fixed [`API_ORDER`] order and
    /// restricted to the host platform, each with its best-effort version
    /// (FR-020, FR-021).
    apis: Vec<GpuApi>,
}

/// A single supported graphics API and its best-effort version (FR-021).
///
/// `version` is the API's own version string (e.g. `1.4.354` for Vulkan,
/// `4.6` for OpenGL, `3.2` for OpenGL ES) where the host can supply it, or the
/// `unknown` placeholder otherwise (FR-011). The version is read from read-only
/// runtime artefacts and, by default, from the timeout-bounded vendor
/// diagnostics allowlist (`probe: false` disables the latter).
#[derive(Debug, Clone, Serialize)]
struct GpuApi {
    /// API name from the fixed [`API_ORDER`] vocabulary, e.g. `Vulkan`.
    name: String,
    /// API version string, e.g. `1.4.354`, or `unknown`.
    version: String,
}

/// A single graphics adapter (spec `osinfo` FR-019).
///
/// Every field falls back to a placeholder (`unknown`, or `0` for
/// `vram_bytes`) when the host cannot supply it (FR-011).
#[derive(Debug, Clone, Serialize)]
struct GpuAdapter {
    /// Best-effort human-readable adapter name, e.g.
    /// `Intel Corporation TigerLake-LP GT2 [Iris Xe Graphics]`, or `unknown`.
    name: String,
    /// Adapter vendor name, e.g. `Intel Corporation`, or `unknown`.
    vendor: String,
    /// PCI vendor id as four lowercase hex digits, e.g. `8086`, or `unknown`.
    vendor_id: String,
    /// PCI device id as four lowercase hex digits, e.g. `9a49`, or `unknown`.
    device_id: String,
    /// Kernel driver bound to the adapter, e.g. `i915`, or `unknown`.
    driver: String,
    /// Classified adapter kind: `integrated`, `discrete`, `virtual`, or
    /// `unknown`.
    kind: String,
    /// Dedicated video memory in bytes, or `0` when the host exposes none.
    vram_bytes: u64,
}

/// Physical-hardware identity report (spec `osinfo` FR-022).
///
/// System/chassis identity, motherboard, and firmware (BIOS) values come from
/// the read-only Linux DMI sysfs tree (`/sys/class/dmi/id`); storage devices
/// from `/sys/block`; network interfaces from `/sys/class/net`. A host whose
/// DMI table is absent (or a non-Linux platform) reports the `unknown`
/// placeholder and empty device lists, so the schema is stable everywhere
/// (FR-011). No serial numbers, UUIDs, or asset tags are read (FR-025).
#[derive(Debug, Clone, Serialize)]
struct HardwareSection {
    /// System (product) vendor, e.g. `LENOVO`, or `unknown`.
    system_vendor: String,
    /// System (product) model name, e.g. `20W1S20H00`, or `unknown`.
    system_model: String,
    /// System product version/marketing name, e.g. `ThinkPad T14 Gen 2i`.
    system_version: String,
    /// Product family, e.g. `ThinkPad T14 Gen 2i`, or `unknown`.
    product_family: String,
    /// Chassis type as a human label, e.g. `Notebook`, or `unknown`.
    chassis_type: String,
    /// Chassis vendor, e.g. `LENOVO`, or `unknown`.
    chassis_vendor: String,
    /// Motherboard (baseboard) vendor, e.g. `LENOVO`, or `unknown`.
    board_vendor: String,
    /// Motherboard (baseboard) model, e.g. `20W1S20H00`, or `unknown`.
    board_name: String,
    /// Motherboard (baseboard) version, e.g. `SDK0J40709 WIN`, or `unknown`.
    board_version: String,
    /// Firmware (BIOS) vendor, e.g. `LENOVO`, or `unknown`.
    bios_vendor: String,
    /// Firmware (BIOS) version, e.g. `N34ET71W (1.71 )`, or `unknown`.
    bios_version: String,
    /// Firmware (BIOS) release date, e.g. `05/09/2026`, or `unknown`.
    bios_date: String,
    /// Detected physical (non-virtual) block devices.
    storage: Vec<StorageDevice>,
    /// Detected network interfaces.
    network: Vec<NetworkInterface>,
}

/// A single physical storage device (spec `osinfo` FR-023).
///
/// One entry per non-virtual `/sys/block` node, ordered by kernel name so the
/// list is stable across calls. Devices with no readable model/vendor report the
/// `unknown` placeholder rather than being dropped (FR-011).
#[derive(Debug, Clone, Serialize)]
struct StorageDevice {
    /// Kernel block-device name, e.g. `nvme0n1` or `sda`.
    name: String,
    /// Best-effort model, e.g. `SOLIDIGM SSDPFKNU020TZ`, or `unknown`.
    model: String,
    /// Device vendor, e.g. `Seagate`, or `unknown`.
    vendor: String,
    /// Device capacity in bytes (`0` when the host exposes none).
    size_bytes: u64,
    /// Coarse kind: `nvme`, `ssd`, `hdd`, `virtual`, or `unknown`.
    kind: String,
}

/// A single network interface (spec `osinfo` FR-024).
///
/// One entry per `/sys/class/net` node, including the loopback interface,
/// ordered by name so the list is stable across calls.
#[derive(Debug, Clone, Serialize)]
struct NetworkInterface {
    /// Interface name, e.g. `wlp0s20f3`.
    name: String,
    /// MAC address, e.g. `96:e6:55:71:27:86`, or `unknown`.
    mac_address: String,
    /// Operational state, e.g. `up`, `down`, or `unknown`.
    oper_state: String,
    /// Link speed in Mbit/s, or `0` when unknown/unsupported.
    speed_mbps: u32,
    /// True for the loopback interface.
    is_loopback: bool,
}

/// Physical memory, swap, and uptime report (spec `osinfo` FR-006).
///
/// Byte counts are exact; the text renderer adds GiB and days/hours/minutes
/// renderings.
#[derive(Debug, Clone, Serialize)]
struct MemorySection {
    /// Total physical memory in bytes.
    total_memory_bytes: u64,
    /// Currently available physical memory in bytes.
    available_memory_bytes: u64,
    /// Total swap space in bytes.
    total_swap_bytes: u64,
    /// Currently free swap space in bytes.
    free_swap_bytes: u64,
    /// System uptime in seconds.
    uptime_seconds: u64,
    /// Boot time as an RFC 3339 UTC timestamp, or `unknown`.
    boot_time_utc: String,
}

/// ragent process environment report (spec `osinfo` FR-007).
///
/// No environment variable other than the resolved shell path is exposed
/// (FR-008); every field falls back to `unknown` when it cannot be resolved.
#[derive(Debug, Clone, Serialize)]
struct ProcessSection {
    /// Process id of the running ragent process.
    pid: u32,
    /// Current working directory of the ragent process.
    working_directory: String,
    /// Resolved user shell (`$SHELL` on Unix, `COMSPEC` on Windows).
    shell: String,
    /// Username running ragent, or `unknown`.
    username: String,
}

/// Normalized host information returned by [`OsInfoTool`].
///
/// Grouped into one section per text-format heading so the tool and the
/// `/osinfo show` slash command share a single schema (FR-018).
#[derive(Debug, Clone, Serialize)]
pub struct OsInfo {
    /// Operating-system identity (FR-002, FR-003).
    os: OsSection,
    /// CPU architecture and capability (FR-005).
    cpu: CpuSection,
    /// Graphics adapters (FR-019).
    gpu: GpuSection,
    /// Physical-hardware identity, storage, and network (FR-022, FR-023, FR-024).
    hardware: HardwareSection,
    /// Memory, swap, and uptime (FR-006).
    memory: MemorySection,
    /// Process environment (FR-007).
    process: ProcessSection,
}

/// Read-only tool that reports host operating-system and hardware information.
pub struct OsInfoTool;

/// Placeholder used when a platform value cannot be determined (FR-002, FR-003).
const UNKNOWN: &str = "unknown";

/// The accepted values for the optional `format` parameter (FR-004).
const FORMATS: &[&str] = &["text", "json"];

#[async_trait::async_trait]
impl Tool for OsInfoTool {
    fn name(&self) -> &'static str {
        "os_info"
    }

    fn description(&self) -> &'static str {
        "Report read-only information about the host operating system and hardware: \
         OS identity (family, name, version, kernel, hostname and, on Linux, the \
         distribution id and name), CPU (architecture, physical and logical cores, \
         vendor, brand, frequency in MHz), graphics adapters (integrated and \
         discrete: vendor, PCI ids, driver, class and VRAM where exposed) and \
         graphics-API support with version numbers (Direct3D, DirectX, Metal, \
         OpenGL, OpenGL ES, Mesa, Vulkan, OptiX, CUDA, ROCm), physical hardware \
         (system vendor/model/version and family, chassis type and vendor, \
         motherboard vendor/name/version, BIOS vendor/version/date, and on Linux \
         the detected physical storage devices and network interfaces - never \
         serial numbers, UUIDs, or asset tags), memory \
         and swap (exact bytes, with GiB \
         renderings), system uptime and boot time, and the ragent process \
         environment (pid, working directory, shell, username). Accepts an \
         optional format ('text' - the default, human-readable markdown - or \
         'json') and an optional boolean 'probe' (default true). By default every \
         graphics API is reported with a best-effort version: read from the host's \
         own configuration artefacts (Vulkan ICD manifests, CUDA/ROCm version \
         files) and from a fixed, timeout-bounded set of vendor diagnostics \
         (vulkaninfo, glxinfo, nvidia-smi, rocminfo, system_profiler) run so APIs \
         whose version only a live diagnostic reports (OpenGL, OpenGL ES, Mesa) \
         are covered too; processes are spawned only for those allowlisted \
         diagnostics and the call never writes files, makes network requests, or \
         reads secret material (no environment-variable dump). Setting 'probe: \
         false' skips the diagnostics for a fully process-free pass. The \
         schema sets additionalProperties: false. It always succeeds, reporting \
         'unknown'/'0' placeholders for any value the host cannot supply."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "format": {
                    "type": "string",
                    "enum": FORMATS,
                    "description": "Output format: 'text' (human-readable markdown, default) or 'json' (structured metadata only)"
                },
                "probe": {
                    "type": "boolean",
                    "description": "When true (the default), run a fixed, timeout-bounded set of vendor diagnostics to confirm graphics-API support and read each version (needed for OpenGL, OpenGL ES, and Mesa, which expose no read-only version file). Set false for a fully process-free pass that relies on installed runtimes and read-only version files only."
                }
            },
            "additionalProperties": false
        })
    }

    fn permission_category(&self) -> &'static str {
        "none"
    }

    /// # Errors
    ///
    /// Returns an error only when `format` is supplied but is not one of
    /// `text` or `json`; all other outcomes succeed with a placeholder for any
    /// value the host cannot supply (FR-004, FR-011, FR-020).
    async fn execute(&self, input: Value, _ctx: &ToolContext) -> Result<ToolOutput> {
        let format = match input.get("format") {
            None | Some(Value::Null) => "text",
            Some(Value::String(value)) => value.as_str(),
            Some(other) => {
                return Err(invalid_format(&other.to_string()));
            }
        };

        let probe = input.get("probe").and_then(Value::as_bool).unwrap_or(true);
        let info = OsInfo::collect_with_probe(probe);

        match format {
            "text" => Ok(ToolOutput {
                content: render_text(&info),
                metadata: Some(render_json(&info)),
            }),
            "json" => {
                let value = render_json(&info);
                Ok(ToolOutput {
                    content: serde_json::to_string_pretty(&value)?,
                    metadata: Some(value),
                })
            }
            other => Err(invalid_format(other)),
        }
    }
}

/// Build the error returned for an unrecognised `format` value (FR-004).
///
/// Names the offending value and lists the accepted ones so the agent can
/// correct itself instead of inventing data (TC-004).
fn invalid_format(value: &str) -> anyhow::Error {
    anyhow::anyhow!(
        "invalid 'format' value '{value}': accepted values are {}",
        FORMATS.join(", ")
    )
}

impl OsInfo {
    /// Collect the host operating-system, hardware, and process information.
    ///
    /// Every subsection is gathered independently and falls back to a
    /// placeholder (`unknown`, `n/a (<family>)`, or `0`) rather than failing,
    /// so the collector always succeeds (FR-011). This is the single source of
    /// truth shared by the `os_info` tool and the `/osinfo show` slash command
    /// (FR-018). Graphics-API support and version come from installed runtimes
    /// plus the default, timeout-bounded vendor diagnostics, so an API whose
    /// version only a live diagnostic reports (OpenGL, OpenGL ES, Mesa) is
    /// described too (FR-020, FR-021). This is the default surface and is
    /// equivalent to calling `collect_with_probe(true)`.
    pub fn collect() -> Self {
        Self::collect_with_probe(true)
    }

    /// Collect the host report, optionally confirming graphics-API support by
    /// running the vendor diagnostics allowlist (FR-020).
    ///
    /// `probe` is `true` by default: a fixed, timeout-bounded allowlist of
    /// diagnostics (`vulkaninfo`, `glxinfo`, `nvidia-smi`, `rocminfo`,
    /// `system_profiler`) is run to confirm support and read the version, which
    /// is how OpenGL, OpenGL ES, and Mesa acquire a version at all (FR-021).
    /// When `probe` is `false` the pass is confined to installed runtimes and
    /// read-only version files and spawns no process.
    /// Every failure and timeout degrades to "no signal" rather than failing
    /// the call (FR-011).
    pub fn collect_with_probe(probe: bool) -> Self {
        Self {
            os: OsSection::collect(),
            cpu: CpuSection::collect(),
            gpu: GpuSection::collect(probe),
            hardware: HardwareSection::collect(),
            memory: MemorySection::collect(),
            process: ProcessSection::collect(),
        }
    }
}

/// Return `Some` for a value that carries content, `None` when absent or blank.
///
/// `sysinfo` sometimes yields an empty string instead of `None`; both are
/// treated as "not determined" so the placeholder logic in [`fallback`] applies.
fn non_empty(value: Option<String>) -> Option<String> {
    value.filter(|v| !v.trim().is_empty())
}

/// Coerce an optional string to its value, or the `unknown` placeholder.
fn fallback(value: Option<String>) -> String {
    non_empty(value).unwrap_or_else(|| UNKNOWN.to_string())
}

/// Render a byte count as GiB to two decimal places (FR-006).
fn gib(bytes: u64) -> String {
    format!("{:.2} GiB", bytes as f64 / 1_073_741_824.0)
}

/// Render a second count as days/hours/minutes (FR-006).
fn human_uptime(seconds: u64) -> String {
    let days = seconds / 86_400;
    let hours = (seconds % 86_400) / 3_600;
    let minutes = (seconds % 3_600) / 60;
    format!("{days}d {hours}h {minutes}m")
}

/// Render the collected host information as a human-readable markdown report.
///
/// One section per category (`Operating System`, `CPU`, `GPU`, `Memory`,
/// `Process`) mirrors the JSON structure; byte counts gain a GiB rendering and
/// uptime a days/hours/minutes rendering (FR-004, FR-006). When no graphics
/// adapter is detected the `GPU` section renders a single `unknown` line so the
/// section is always present and the schema stays stable (FR-019).
///
/// `pub` so the TUI `/osinfo show` handler renders the same report as the
/// tool's `format: text` output rather than reimplementing it (FR-016, FR-018).
#[must_use]
pub fn render_text(info: &OsInfo) -> String {
    let os = &info.os;
    let cpu = &info.cpu;
    let mem = &info.memory;
    let proc = &info.process;

    let distribution = distribution_label(os);

    let mut lines = vec![
        "## Operating System".to_string(),
        format!("- **Family**: {}", os.family),
        format!("- **Name**: {}", os.name),
        format!("- **Version**: {}", os.version),
        format!("- **Long Version**: {}", os.long_version),
        format!("- **Kernel**: {}", os.kernel_version),
        format!("- **Hostname**: {}", os.hostname),
        format!("- **Distribution**: {distribution}"),
        String::new(),
        "## CPU".to_string(),
        format!("- **Architecture**: {}", cpu.architecture),
        format!("- **Physical Cores**: {}", cpu.physical_cores),
        format!("- **Logical Cores**: {}", cpu.logical_cores),
        format!("- **Vendor**: {}", cpu.vendor),
        format!("- **Brand**: {}", cpu.brand),
        format!("- **Frequency**: {} MHz", cpu.frequency_mhz),
        String::new(),
    ];
    lines.extend(gpu_text_lines(&info.gpu));
    lines.extend(hardware_text_lines(&info.hardware));
    lines.extend([
        String::new(),
        "## Memory".to_string(),
        format!(
            "- **Total Memory**: {} bytes ({})",
            mem.total_memory_bytes,
            gib(mem.total_memory_bytes)
        ),
        format!(
            "- **Available Memory**: {} bytes ({})",
            mem.available_memory_bytes,
            gib(mem.available_memory_bytes)
        ),
        format!(
            "- **Total Swap**: {} bytes ({})",
            mem.total_swap_bytes,
            gib(mem.total_swap_bytes)
        ),
        format!(
            "- **Free Swap**: {} bytes ({})",
            mem.free_swap_bytes,
            gib(mem.free_swap_bytes)
        ),
        format!(
            "- **Uptime**: {} seconds ({})",
            mem.uptime_seconds,
            human_uptime(mem.uptime_seconds)
        ),
        format!("- **Boot Time (UTC)**: {}", mem.boot_time_utc),
        String::new(),
        "## Process".to_string(),
        format!("- **PID**: {}", proc.pid),
        format!("- **Working Directory**: {}", proc.working_directory),
        format!("- **Shell**: {}", proc.shell),
        format!("- **Username**: {}", proc.username),
    ]);
    lines.join("\n")
}

/// Render the `Hardware` section lines shared by the text renderer
/// (FR-022, FR-023, FR-024).
///
/// Always emits the `## Hardware` heading followed by the system, chassis,
/// motherboard, and firmware bullets, then a `Storage` bullet listing one entry
/// per detected physical device and a `Network` bullet listing one entry per
/// interface. Empty device lists render a `none detected` placeholder so the
/// section keeps a stable shape on every platform (FR-011).
fn hardware_text_lines(hardware: &HardwareSection) -> Vec<String> {
    let mut lines = vec![
        String::new(),
        "## Hardware".to_string(),
        format!("- **System Vendor**: {}", hardware.system_vendor),
        format!("- **System Model**: {}", hardware.system_model),
        format!("- **System Version**: {}", hardware.system_version),
        format!("- **Product Family**: {}", hardware.product_family),
        format!("- **Chassis**: {}", hardware.chassis_type),
        format!("- **Chassis Vendor**: {}", hardware.chassis_vendor),
        format!("- **Board Vendor**: {}", hardware.board_vendor),
        format!("- **Board Name**: {}", hardware.board_name),
        format!("- **Board Version**: {}", hardware.board_version),
        format!("- **BIOS Vendor**: {}", hardware.bios_vendor),
        format!("- **BIOS Version**: {}", hardware.bios_version),
        format!("- **BIOS Date**: {}", hardware.bios_date),
    ];

    if hardware.storage.is_empty() {
        lines.push("- **Storage**: none detected".to_string());
    } else {
        lines.push("- **Storage**:".to_string());
        for device in &hardware.storage {
            lines.push(format!(
                "  - **{}**: {} {} ({} bytes, {})",
                device.name, device.vendor, device.model, device.size_bytes, device.kind
            ));
        }
    }

    if hardware.network.is_empty() {
        lines.push("- **Network**: none detected".to_string());
    } else {
        lines.push("- **Network**:".to_string());
        for iface in &hardware.network {
            let speed = if iface.speed_mbps > 0 {
                format!("{} Mbit/s", iface.speed_mbps)
            } else {
                UNKNOWN.to_string()
            };
            lines.push(format!(
                "  - **{}**: {} (state {}, speed {}, loopback {})",
                iface.name, iface.mac_address, iface.oper_state, speed, iface.is_loopback
            ));
        }
    }

    lines
}

/// Render the `GPU` section lines shared by the text renderer (FR-019, FR-020).
///
/// Always emits the `## GPU` heading: the `- **APIs**` line from
/// [`graphics_apis_line`], then one bullet per adapter when any is detected,
/// otherwise a single `- **Adapters**: unknown` line so the section is present
/// on every platform and the surrounding report keeps a stable shape (FR-011,
/// FR-019, FR-020).
fn gpu_text_lines(gpu: &GpuSection) -> Vec<String> {
    let mut lines = vec!["## GPU".to_string()];
    lines.push(graphics_apis_line(&gpu.apis));
    if gpu.adapters.is_empty() {
        lines.push(format!("- **Adapters**: {UNKNOWN}"));
        return lines;
    }
    for adapter in &gpu.adapters {
        lines.push(format!("- **Adapter**: {}", adapter.name));
        lines.push(format!("  - **Vendor**: {}", adapter.vendor));
        lines.push(format!(
            "  - **PCI Ids**: {}:{}",
            adapter.vendor_id, adapter.device_id
        ));
        lines.push(format!("  - **Driver**: {}", adapter.driver));
        lines.push(format!("  - **Kind**: {}", adapter.kind));
        if adapter.vram_bytes > 0 {
            lines.push(format!(
                "  - **VRAM**: {} bytes ({})",
                adapter.vram_bytes,
                gib(adapter.vram_bytes)
            ));
        }
    }
    lines
}

/// Render the `- **APIs**` line listing the detected graphics APIs (FR-020).
///
/// Produces `- **APIs**: none detected` when the list is empty and otherwise
/// joins one rendered entry per API in [`API_ORDER`] order.
fn graphics_apis_line(apis: &[GpuApi]) -> String {
    if apis.is_empty() {
        return "- **APIs**: none detected".to_string();
    }
    let rendered = apis.iter().map(render_api).collect::<Vec<_>>().join(", ");
    format!("- **APIs**: {rendered}")
}

/// Render one graphics API for the text `- **APIs**` line (FR-021).
///
/// Appends the version when it is known (`Vulkan 1.4.354`), and shows the bare
/// name when the host could not supply a version, so the line never carries a
/// misleading `unknown` token next to a real API name.
fn render_api(api: &GpuApi) -> String {
    if api.version == UNKNOWN {
        api.name.clone()
    } else {
        format!("{} {}", api.name, api.version)
    }
}

/// Build the distribution label shared by the text and JSON renderers.
///
/// On Linux this is `<id> / <display-name>`; on every other platform the
/// collector already stores the shared `n/a (<family>)` placeholder, so it is
/// returned unchanged (FR-003, FR-018).
fn distribution_label(os: &OsSection) -> String {
    if os.family == "linux" {
        format!("{} / {}", os.distribution_id, os.distribution_name)
    } else {
        os.distribution_id.clone()
    }
}

/// Render the collected host information as one flat JSON object with stable
/// snake_case keys (FR-004).
///
/// The layout is deliberately flat - unlike the nested collector structs - so
/// the key set the agent sees stays fixed on every platform and each field of
/// the text report has a same-named JSON counterpart. `distribution` carries
/// the same combined label the text renderer prints.
fn render_json(info: &OsInfo) -> Value {
    let os = &info.os;
    let cpu = &info.cpu;
    let hardware = &info.hardware;
    let mem = &info.memory;
    let proc = &info.process;

    json!({
        // Operating-system identity (FR-002, FR-003).
        "os_family": os.family.clone(),
        "os_name": os.name.clone(),
        "os_version": os.version.clone(),
        "os_long_version": os.long_version.clone(),
        "kernel_version": os.kernel_version.clone(),
        "hostname": os.hostname.clone(),
        "distribution": distribution_label(os),
        // CPU architecture and capability (FR-005).
        "cpu_arch": cpu.architecture.clone(),
        "physical_cores": cpu.physical_cores,
        "logical_cores": cpu.logical_cores,
        "cpu_vendor": cpu.vendor.clone(),
        "cpu_brand": cpu.brand.clone(),
        "cpu_frequency_mhz": cpu.frequency_mhz,
        // Graphics adapters and graphics-API support (FR-019, FR-020, FR-021).
        "gpus": gpu_json_adapters(&info.gpu),
        "gpu_apis": gpu_json_apis(&info.gpu),
        // Physical-hardware identity, storage, and network (FR-022..FR-024).
        "system_vendor": hardware.system_vendor.clone(),
        "system_model": hardware.system_model.clone(),
        "system_version": hardware.system_version.clone(),
        "product_family": hardware.product_family.clone(),
        "chassis_type": hardware.chassis_type.clone(),
        "chassis_vendor": hardware.chassis_vendor.clone(),
        "board_vendor": hardware.board_vendor.clone(),
        "board_name": hardware.board_name.clone(),
        "board_version": hardware.board_version.clone(),
        "bios_vendor": hardware.bios_vendor.clone(),
        "bios_version": hardware.bios_version.clone(),
        "bios_date": hardware.bios_date.clone(),
        "storage_devices": storage_json(&hardware.storage),
        "network_interfaces": network_json(&hardware.network),
        // Memory, swap, and uptime (FR-006).
        "total_memory_bytes": mem.total_memory_bytes,
        "available_memory_bytes": mem.available_memory_bytes,
        "total_swap_bytes": mem.total_swap_bytes,
        "free_swap_bytes": mem.free_swap_bytes,
        "uptime_seconds": mem.uptime_seconds,
        "boot_time_utc": mem.boot_time_utc.clone(),
        // Process environment (FR-007).
        "pid": proc.pid,
        "working_directory": proc.working_directory.clone(),
        "shell": proc.shell.clone(),
        "username": proc.username.clone(),
    })
}

/// Build the `gpus` JSON array from the collected adapters (FR-019).
///
/// Always an array (empty when no adapter is detected) so consumers can index
/// it uniformly across platforms. Each element is a flat object whose keys
/// mirror the text bullets (`name`, `vendor`, `vendor_id`, `device_id`,
/// `driver`, `kind`, `vram_bytes`).
fn gpu_json_adapters(gpu: &GpuSection) -> Vec<Value> {
    gpu.adapters
        .iter()
        .map(|adapter| {
            json!({
                "name": adapter.name.clone(),
                "vendor": adapter.vendor.clone(),
                "vendor_id": adapter.vendor_id.clone(),
                "device_id": adapter.device_id.clone(),
                "driver": adapter.driver.clone(),
                "kind": adapter.kind.clone(),
                "vram_bytes": adapter.vram_bytes,
            })
        })
        .collect()
}

/// Build the `gpu_apis` JSON array from the collected APIs (FR-020, FR-021).
///
/// Always an array (empty when no API is detected) whose elements are objects
/// with `name` and `version` keys, matching the text `- **APIs**` line
/// (`Vulkan 1.4.354`). `version` is the `unknown` placeholder when no source
/// carried a version.
fn gpu_json_apis(gpu: &GpuSection) -> Vec<Value> {
    gpu.apis
        .iter()
        .map(|api| {
            json!({
                "name": api.name.clone(),
                "version": api.version.clone(),
            })
        })
        .collect()
}

/// Build the `storage_devices` JSON array from the collected devices (FR-023).
///
/// Always an array (empty when no device is detected) whose elements are objects
/// with `name`, `model`, `vendor`, `size_bytes`, and `kind` keys, mirroring the
/// text `Storage` bullets.
fn storage_json(devices: &[StorageDevice]) -> Vec<Value> {
    devices
        .iter()
        .map(|device| {
            json!({
                "name": device.name.clone(),
                "model": device.model.clone(),
                "vendor": device.vendor.clone(),
                "size_bytes": device.size_bytes,
                "kind": device.kind.clone(),
            })
        })
        .collect()
}

/// Build the `network_interfaces` JSON array from the collected interfaces
/// (FR-024).
///
/// Always an array (empty when no interface is detected) whose elements are
/// objects with `name`, `mac_address`, `oper_state`, `speed_mbps`, and
/// `is_loopback` keys, mirroring the text `Network` bullets.
fn network_json(interfaces: &[NetworkInterface]) -> Vec<Value> {
    interfaces
        .iter()
        .map(|iface| {
            json!({
                "name": iface.name.clone(),
                "mac_address": iface.mac_address.clone(),
                "oper_state": iface.oper_state.clone(),
                "speed_mbps": iface.speed_mbps,
                "is_loopback": iface.is_loopback,
            })
        })
        .collect()
}

/// Render a Unix epoch second count as an RFC 3339 UTC timestamp.
///
/// Uses the `Z` (Zulu) UTC designator with second precision. Returns the
/// `unknown` placeholder for a value the timestamp type cannot represent
/// (FR-006, FR-011).
fn u64_to_rfc3339(secs: u64) -> String {
    let secs = i64::try_from(secs).ok();
    secs.and_then(|s| chrono::DateTime::from_timestamp(s, 0))
        .map_or_else(
            || UNKNOWN.to_string(),
            |dt| dt.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        )
}

/// Map a `sysinfo` / `std::env` OS identifier to a stable lowercase family name.
///
/// The result is one of `linux`, `macos`, `windows`, `freebsd`, or `unknown`,
/// matching the vocabulary the spec exposes (FR-002).
fn os_family(raw: &str) -> String {
    match raw.trim().to_ascii_lowercase().as_str() {
        "" => UNKNOWN.to_string(),
        "linux" => "linux".to_string(),
        "macos" | "darwin" | "apple" => "macos".to_string(),
        "windows" => "windows".to_string(),
        "freebsd" => "freebsd".to_string(),
        other => other.to_string(),
    }
}

/// Read the `PRETTY_NAME` display string from `/etc/os-release`.
///
/// Linux-only convenience used for the distribution display name (FR-003).
/// Returns `None` on any platform where the file is absent or the key is
/// missing; the caller supplies the placeholder.
fn linux_distribution_name() -> Option<String> {
    let contents = std::fs::read_to_string("/etc/os-release").ok()?;
    contents.lines().find_map(|line| {
        line.strip_prefix("PRETTY_NAME=")
            .map(|value| value.trim().trim_matches('"').to_string())
            .filter(|value| !value.is_empty())
    })
}

impl OsSection {
    /// Collect the OS identity fields (FR-002, FR-003).
    ///
    /// Never fails: any value the host cannot supply becomes `unknown`, and
    /// the distribution fields become `n/a (<family>)` off Linux.
    fn collect() -> Self {
        let family = os_family(std::env::consts::OS);

        let distribution_id = if family == "linux" {
            sysinfo::System::distribution_id()
        } else {
            String::new()
        };
        let distribution_name = if family == "linux" {
            linux_distribution_name().unwrap_or_default()
        } else {
            String::new()
        };

        // FR-003: off Linux (or when the distro is undetermined) the fields
        // carry the `n/a (<family>)` placeholder so the schema is stable.
        let not_applicable = || format!("n/a ({family})");
        let distribution_id = if distribution_id.trim().is_empty() {
            not_applicable()
        } else {
            distribution_id
        };
        let distribution_name = if distribution_name.trim().is_empty() {
            not_applicable()
        } else {
            distribution_name
        };

        Self {
            family,
            name: fallback(sysinfo::System::name()),
            version: fallback(sysinfo::System::os_version()),
            long_version: fallback(sysinfo::System::long_os_version()),
            kernel_version: fallback(sysinfo::System::kernel_version()),
            hostname: fallback(sysinfo::System::host_name()),
            distribution_id,
            distribution_name,
        }
    }
}

impl CpuSection {
    /// Collect the CPU capability report (FR-005, FR-011).
    ///
    /// Never fails: a metric the host cannot supply becomes `unknown` (vendor,
    /// brand) or `0` (core counts, frequency). Values are read from the first
    /// logical CPU, which is representative on symmetric multi-core hosts.
    fn collect() -> Self {
        let mut system = sysinfo::System::new();
        system.refresh_cpu_all();
        let cpus = system.cpus();
        let first = cpus.first();

        Self {
            architecture: sysinfo::System::cpu_arch(),
            physical_cores: sysinfo::System::physical_core_count().unwrap_or(0),
            logical_cores: cpus.len(),
            vendor: first.map_or_else(
                || UNKNOWN.to_string(),
                |cpu| fallback(Some(cpu.vendor_id().to_string())),
            ),
            brand: first.map_or_else(
                || UNKNOWN.to_string(),
                |cpu| fallback(Some(cpu.brand().to_string())),
            ),
            frequency_mhz: first.map_or(0, sysinfo::Cpu::frequency),
        }
    }
}

impl GpuSection {
    /// Collect the graphics-adapter and graphics-API report (FR-019, FR-020, FR-011).
    ///
    /// Linux reads the read-only DRM sysfs tree under `/sys/class/drm`: each
    /// `cardN` node contributes one adapter, resolved to its bound kernel driver
    /// and PCI ids. Non-Linux platforms (and Linux hosts with no DRM cards)
    /// yield an empty adapter list rather than failing the call.
    ///
    /// Graphics-API support is always inferred from installed runtimes
    /// (`inferred_api_supported`), which never spawns a process, writes a file,
    /// or touches the network (FR-009). When `probe` is `true` the inference is
    /// widened by the fixed, timeout-bounded vendor diagnostics allowlist
    /// (`probed_api_supported`), which spawns the allowlisted commands only;
    /// this is on by default because OpenGL, OpenGL ES, and Mesa expose no
    /// read-only version artefact (FR-020, FR-021).
    fn collect(probe: bool) -> Self {
        let adapters = if cfg!(target_os = "linux") {
            collect_linux_adapters()
        } else {
            Vec::new()
        };
        let apis = api_support(probe);
        Self { adapters, apis }
    }
}

/// Order in which detected graphics APIs are reported (FR-020).
///
/// Direct3D/DirectX are Windows-only, Metal is macOS-only; the remainder are
/// filtered per platform by [`api_supported`].
const API_ORDER: &[&str] = &[
    "Direct3D",
    "DirectX",
    "Metal",
    "OpenGL",
    "OpenGL ES",
    "Mesa",
    "Vulkan",
    "OptiX",
    "CUDA",
    "ROCm",
];

/// The graphics APIs this platform can in principle support (FR-020).
///
/// Direct3D and DirectX are reported only on Windows, and Metal only on macOS;
/// every other API is relevant on all platforms.
fn platform_apis() -> Vec<&'static str> {
    API_ORDER
        .iter()
        .copied()
        .filter(|api| api_supported(api))
        .collect()
}

/// Whether an API is in scope for the host platform (FR-020).
fn api_supported(api: &str) -> bool {
    let family = os_family(std::env::consts::OS);
    match api {
        "Direct3D" | "DirectX" => family == "windows",
        "Metal" => family == "macos",
        _ => true,
    }
}

/// Detect graphics-API support and versions, in [`API_ORDER`] order
/// (FR-020, FR-021).
///
/// Inference from installed runtimes always runs; `probe` additionally enables
/// the vendor-diagnostics allowlist. An API is reported when either signal is
/// present. Support is first established (filesystem inference, or a probe when
/// enabled), then the API's version is resolved from the read-only artefacts,
/// refined by any probe that printed one (FR-021).
fn api_support(probe: bool) -> Vec<GpuApi> {
    platform_apis()
        .into_iter()
        .filter(|api| inferred_api_supported(api) || (probe && probed_api_supported(api)))
        .map(|api| GpuApi {
            name: api.to_string(),
            version: api_version(api, probe),
        })
        .collect()
}

/// Filesystem inference for one API: runtime libraries, ICD manifests, or
/// vendor tools present under well-known locations (FR-020).
///
/// Read-only; never spawns a process. Returns `false` when no known file exists
/// (the report then omits the API rather than guessing).
fn inferred_api_supported(api: &str) -> bool {
    match api {
        // Windows system libraries back Direct3D/DirectX.
        "Direct3D" | "DirectX" => any_exists(&[
            r"C:\Windows\System32\d3d11.dll",
            r"C:\Windows\System32\dxgi.dll",
        ]),
        // Metal ships as a macOS system framework.
        "Metal" => any_exists(&[
            "/System/Library/Frameworks/Metal.framework",
            "/System/Library/Frameworks/Metal.framework/Metal",
        ]),
        // OpenGL/OpenGL ES driver libraries.
        "OpenGL" => any_exists(&[
            "/usr/lib/x86_64-linux-gnu/libGL.so.1",
            "/usr/lib/aarch64-linux-gnu/libGL.so.1",
            "/usr/lib64/libGL.so.1",
            "/usr/lib64/libGL.so",
            "/usr/lib/libGL.so.1",
            "/usr/lib/libGL.so",
            "/usr/lib/libGL.dylib",
            "/System/Library/Frameworks/OpenGL.framework/OpenGL",
            r"C:\Windows\System32\opengl32.dll",
        ]),
        "OpenGL ES" => any_exists(&[
            "/usr/lib/x86_64-linux-gnu/libGLESv2.so.2",
            "/usr/lib/aarch64-linux-gnu/libGLESv2.so.2",
            "/usr/lib64/libGLESv2.so.2",
            "/usr/lib/libGLESv2.so.2",
            "/usr/lib/libGLESv2.dylib",
        ]),
        // Mesa's userspace Vulkan/GL stack.
        "Mesa" => any_exists(&[
            "/usr/lib/x86_64-linux-gnu/libvulkan_mesa.so",
            "/usr/lib/x86_64-linux-gnu/libGLX_mesa.so.0",
            "/usr/lib/x86_64-linux-gnu/libEGL_mesa.so.0",
            "/usr/lib64/libvulkan_mesa.so",
            "/usr/lib64/libGLX_mesa.so.0",
        ]),
        // Vulkan loader plus ICD manifests (any one manifest implies a driver).
        "Vulkan" => {
            any_exists(&[
                "/usr/share/vulkan/icd.d",
                "/etc/vulkan/icd.d",
                "/usr/share/vulkan/icd.d/nvidia_icd.json",
                "/usr/share/vulkan/icd.d/radeon_icd.x86_64.json",
                "/usr/share/vulkan/icd.d/intel_icd.x86_64.json",
                "/usr/share/vulkan/icd.d/lvp_icd.x86_64.json",
                r"C:\Windows\System32\vulkan-1.dll",
            ]) || any_dir_has_file(&["/usr/share/vulkan/icd.d", "/etc/vulkan/icd.d"])
        }
        // OptiX (NVIDIA ray tracing) ships under the CUDA SDK or the driver.
        "OptiX" => any_exists(&[
            "/usr/local/cuda/lib64/libnvoptix.so.1",
            "/usr/lib/x86_64-linux-gnu/libnvoptix.so.1",
            "/usr/lib64/libnvoptix.so.1",
            "/opt/nvidia/lib/libnvoptix.so.1",
        ]),
        // CUDA toolkit or driver runtime library.
        "CUDA" => any_exists(&[
            "/usr/local/cuda/bin/nvcc",
            "/usr/local/cuda/lib64/libcudart.so",
            "/usr/lib/x86_64-linux-gnu/libcuda.so.1",
            "/usr/lib/x86_64-linux-gnu/libcudart.so",
            "/usr/lib64/libcuda.so.1",
            "/usr/lib64/libcudart.so",
            r"C:\Program Files\NVIDIA GPU Computing Toolkit\CUDA",
        ]),
        // ROCm/HIP runtime library.
        "ROCm" => any_exists(&[
            "/opt/rocm/bin/rocminfo",
            "/opt/rocm/lib/libamdhip64.so",
            "/opt/rocm/lib/libhiprtc.so",
            "/usr/lib/x86_64-linux-gnu/libamdhip64.so",
        ]),
        _ => false,
    }
}

/// Whether any of the given paths exists on the host.
fn any_exists(paths: &[&str]) -> bool {
    paths.iter().any(|path| Path::new(path).exists())
}

/// Whether any of the given directories holds at least one entry.
///
/// Used for Vulkan ICD directories: a non-empty ICD directory means a driver
/// manifest is installed even when its filename is unknown to us. An
/// unreadable directory counts as empty.
fn any_dir_has_file(dirs: &[&str]) -> bool {
    dirs.iter()
        .any(|dir| std::fs::read_dir(dir).is_ok_and(|mut entries| entries.next().is_some()))
}

/// Probe result cache: one vendor-diagnostic run shared across APIs and across
/// the whole process, so a single report never spawns the same tool twice.
///
/// Each entry is `(exit-success, captured-output)`; `None` records a tool that
/// could not be spawned or timed out, so a missing diagnostic is attempted once
/// only. The captured text is retained so a single run can confirm support
/// (needle match) and contribute the API version (FR-020, FR-021).
#[allow(clippy::type_complexity)]
fn probe_cache()
-> &'static std::sync::Mutex<Option<std::collections::HashMap<&'static str, Option<ProbeRun>>>> {
    use std::sync::OnceLock;
    static CACHE: OnceLock<
        std::sync::Mutex<Option<std::collections::HashMap<&'static str, Option<ProbeRun>>>>,
    > = OnceLock::new();
    CACHE.get_or_init(|| std::sync::Mutex::new(None))
}

/// Run one diagnostic at most once, returning its cached `(success, text)`.
///
/// A `None` result means the tool could not be spawned or timed out (FR-011).
fn probe_result(probe: &Probe) -> Option<(bool, String)> {
    let mut guard = match probe_cache().lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    let cache = guard.get_or_insert_with(std::collections::HashMap::new);
    if let Some(entry) = cache.get(probe.name) {
        return entry.clone();
    }
    let result = run_probe(probe);
    cache.insert(probe.name, result.clone());
    result
}

/// Run one API's allowlisted diagnostic, returning its cached output (FR-020).
///
/// The diagnostic is run at most once per report and cached for the process
/// lifetime. `None` means there is no probe for the API, or the tool could not
/// be spawned or timed out (FR-011). `success` reflects the process exit status;
/// the text is the combined stdout and stderr.
fn probe_output(api: &str) -> Option<(bool, String)> {
    let probe = probe_for(api)?;
    probe_result(&probe)
}

/// Confirm one API by running its allowlisted vendor diagnostics (FR-020).
///
/// An absent tool, a non-zero exit, or a timeout all mean "no signal" rather
/// than an error (FR-011). Returns `false` when there is nothing to run.
fn probed_api_supported(api: &str) -> bool {
    let Some(probe) = probe_for(api) else {
        return false;
    };
    probe_output(api).is_some_and(|(success, text)| success && text.contains(probe.needle))
}

/// Resolve one API's version from a successful probe's output, or `None` when
/// the probe did not run, failed, or printed no recognisable version (FR-021).
fn probed_api_version(api: &str) -> Option<String> {
    let (success, text) = probe_output(api)?;
    if !success {
        return None;
    }
    parse_probe_version(api, &text)
}

/// A single allowlisted diagnostic command (FR-020).
struct Probe {
    /// Stable cache key (also the command name for diagnostics).
    name: &'static str,
    /// Executable to run.
    program: &'static str,
    /// Arguments passed to the executable.
    args: &'static [&'static str],
    /// Substring that must appear in stdout/stderr for the probe to confirm.
    needle: &'static str,
}

/// One cached diagnostic run: the process exit status and its combined
/// stdout/stderr (FR-020, FR-021).
///
/// The captured text lets a run both confirm support (needle match) and supply
/// the API version; `None` in the probe cache means the tool could not be
/// spawned or timed out.
type ProbeRun = (bool, String);

/// The allowlisted diagnostic for one API, or `None` when the API has no probe
/// (it is then filesystem-inference only) (FR-020).
///
/// `glxinfo -B` backs `OpenGL`, `OpenGL ES`, and `Mesa`: one Mesa diagnostic
/// prints all three versions, and those three APIs expose no read-only version
/// file, so it is the only source of their version and, like the other
/// diagnostics, runs by default (FR-021). The three share the command under
/// distinct cache keys rather than running it once per API. The `needle`
/// confirms support; the version is read from the same captured text (FR-021).
fn probe_for(api: &str) -> Option<Probe> {
    match api {
        "Vulkan" => Some(Probe {
            name: "vulkaninfo",
            program: "vulkaninfo",
            args: &["--summary"],
            needle: "apiVersion",
        }),
        "OpenGL" => Some(Probe {
            name: "glxinfo-opengl",
            program: "glxinfo",
            args: &["-B"],
            needle: "OpenGL version string",
        }),
        "OpenGL ES" => Some(Probe {
            name: "glxinfo-gles",
            program: "glxinfo",
            args: &["-B"],
            needle: "OpenGL ES profile version string",
        }),
        "Mesa" => Some(Probe {
            name: "glxinfo-mesa",
            program: "glxinfo",
            args: &["-B"],
            needle: "Mesa",
        }),
        "CUDA" => Some(Probe {
            name: "nvidia-smi",
            program: "nvidia-smi",
            args: &[],
            needle: "CUDA Version",
        }),
        "OptiX" => Some(Probe {
            name: "nvidia-smi-optix",
            program: "nvidia-smi",
            args: &["--query-gpu=driver_version", "--format=csv,noheader"],
            needle: ".",
        }),
        "ROCm" => Some(Probe {
            name: "rocminfo",
            program: "rocminfo",
            args: &[],
            needle: "ROCm",
        }),
        "Metal" => Some(Probe {
            name: "system_profiler-metal",
            program: "system_profiler",
            args: &["SPDisplaysDataType"],
            needle: "Metal",
        }),
        _ => None,
    }
}

/// The read-only files that carry an API's version, checked in order (FR-021).
///
/// The first file that exists and yields a version is used. Every path is a
/// plain read; a missing file or unparseable content is skipped rather than
/// failing the call (FR-011).
fn api_version_files(api: &str) -> &'static [&'static str] {
    const CUDA_VERSION: [&str; 6] = [
        "/usr/local/cuda/version.json",
        "/usr/local/cuda/version.txt",
        "/opt/cuda/version.json",
        "/opt/cuda/version.txt",
        "/usr/local/cuda/include/cuda.h",
        "/opt/cuda/include/cuda.h",
    ];
    const OPTIX_HEADER: [&str; 4] = [
        "/usr/local/cuda/include/optix.h",
        "/usr/include/optix.h",
        "/opt/cuda/include/optix.h",
        "/opt/nvidia/include/optix.h",
    ];
    const VULKAN_ICD: [&str; 8] = [
        "/usr/share/vulkan/icd.d/nvidia_icd.x86_64.json",
        "/usr/share/vulkan/icd.d/radeon_icd.x86_64.json",
        "/usr/share/vulkan/icd.d/intel_icd.x86_64.json",
        "/etc/vulkan/icd.d/nvidia_icd.json",
        "/etc/vulkan/icd.d/radeon_icd.x86_64.json",
        "/etc/vulkan/icd.d/intel_icd.x86_64.json",
        "/usr/share/vulkan/icd.d/lvp_icd.x86_64.json",
        "/etc/vulkan/icd.d/lvp_icd.x86_64.json",
    ];
    match api {
        "Vulkan" => &VULKAN_ICD,
        "CUDA" => &CUDA_VERSION,
        "OptiX" => &OPTIX_HEADER,
        "ROCm" => &["/opt/rocm/.info/version"],
        "Metal" => &["/System/Library/Frameworks/Metal.framework/Resources/Info.plist"],
        _ => &[],
    }
}

/// Extract a version string from a known marker inside `text` (FR-021).
///
/// The value is the first dotted numeric token that follows `marker`, taken up
/// to the next non-version character. `None` when the marker is absent or is
/// not followed by a version, so unparsed content degrades to `unknown`.
fn find_token_after(text: &str, marker: &str) -> Option<String> {
    let rest = &text[text.find(marker)? + marker.len()..];
    let value: String = rest
        .trim_start_matches(|c: char| !c.is_ascii_digit())
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    let trimmed = value.trim_matches('.');
    (trimmed.contains('.') && trimmed.chars().next().is_some_and(|c| c.is_ascii_digit()))
        .then(|| trimmed.to_string())
}

/// Parse one API's version from a probe's captured output (FR-021).
///
/// Markers are the exact strings the vendor tools print, chosen so a version is
/// only taken when the tool actually reported one; anything else yields `None`.
fn parse_probe_version(api: &str, text: &str) -> Option<String> {
    match api {
        "Vulkan" => find_token_after(text, "apiVersion"),
        "OpenGL" => {
            // The compatibility-profile line gives the highest GL version; fall
            // back to the core-profile capability line when only that is present.
            find_token_after(text, "OpenGL version string:")
                .or_else(|| find_token_after(text, "Max core profile version:"))
        }
        "OpenGL ES" => find_token_after(text, "OpenGL ES profile version string:"),
        "Mesa" => find_token_after(text, "Mesa ").or_else(|| {
            // Some builds print only `Version: <x>` under the renderer block.
            find_token_after(text, "Version:")
        }),
        "CUDA" => find_token_after(text, "CUDA Version:"),
        "ROCm" => find_token_after(text, "ROCm"),
        "Metal" => find_token_after(text, "Metal"),
        _ => None,
    }
}

/// Parse one API's version from a read-only version file (FR-021).
///
/// Vulkan ICD manifests carry a JSON `api_version` string; the remaining APIs
/// carry a plain text version marker. Unrecognised content yields `None`.
fn parse_file_version(api: &str, text: &str) -> Option<String> {
    match api {
        "Vulkan" => find_token_after(text, "\"api_version\""),
        "CUDA" => {
            find_token_after(text, "\"version\"").or_else(|| find_token_after(text, "VERSION"))
        }
        "OptiX" => find_token_after(text, "OPTIX_VERSION"),
        "ROCm" => parse_rocm_version(text),
        "Metal" => find_token_after(text, "CFBundleShortVersionString"),
        _ => None,
    }
}

/// Parse a ROCm version from `/opt/rocm/.info/version` (FR-021).
///
/// The file holds a bare version (e.g. `6.2.0`); any leading tag is skipped.
fn parse_rocm_version(text: &str) -> Option<String> {
    let token = text.split_whitespace().find(|token| {
        token.contains('.') && token.chars().next().is_some_and(|c| c.is_ascii_digit())
    })?;
    let cleaned = token.trim_matches(|c: char| !c.is_ascii_digit() && c != '.');
    (cleaned.contains('.')).then(|| cleaned.to_string())
}

/// Resolve one API's version: read-only file evidence first, then any probe
/// output (FR-021).
///
/// The read-only version files are tried first so the default pass still
/// reports a version on a host that exposes one without running anything; the
/// vendor diagnostic (`probe` true) then supplies the version for an API with
/// no read-only artefact (OpenGL, OpenGL ES, Mesa). Returns the `unknown`
/// placeholder when no source carries a version, so the report always has a
/// value for every detected API (FR-011).
fn api_version(api: &str, probe: bool) -> String {
    for path in api_version_files(api) {
        let Ok(text) = std::fs::read_to_string(path) else {
            continue;
        };
        if let Some(version) = parse_file_version(api, &text) {
            return version;
        }
    }
    if probe && let Some(version) = probed_api_version(api) {
        return version;
    }
    UNKNOWN.to_string()
}

/// Run one allowlisted diagnostic with a hard timeout (FR-020, FR-011).
///
/// The command runs with stdin from `/dev/null` and stdout/stderr captured; a
/// kill-on-timeout guard prevents an orphaned process. Returns
/// `Some((exit_success, combined_output))` once the process exits, or `None`
/// when it could not be spawned, timed out, or the output was not UTF-8. The
/// captured text lets the caller both confirm support (needle match) and read
/// the API version (FR-021). The process environment is inherited but never
/// read into the report, so no secret can leak (FR-008).
fn run_probe(probe: &Probe) -> Option<(bool, String)> {
    let mut command = Command::new(probe.program);
    command
        .args(probe.args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let child = command.spawn().ok()?;
    let output = wait_with_timeout(child, PROBE_TIMEOUT)?;
    let mut combined = String::from_utf8_lossy(&output.stdout).into_owned();
    combined.push('\n');
    combined.push_str(&String::from_utf8_lossy(&output.stderr));
    Some((output.status.success(), combined))
}

/// Timeout for a single vendor diagnostic (FR-020).
const PROBE_TIMEOUT: Duration = Duration::from_secs(2);

/// Wait for a child to exit, killing it once `timeout` elapses.
///
/// Returns `None` on timeout (after killing and reaping the child so nothing is
/// orphaned) and `Some(output)` once the process exits (FR-011).
fn wait_with_timeout(
    mut child: std::process::Child,
    timeout: Duration,
) -> Option<std::process::Output> {
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return child.wait_with_output().ok(),
            Ok(None) => {
                if start.elapsed() >= timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
                std::thread::sleep(Duration::from_millis(25));
            }
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
}

/// Enumerate graphics adapters from the Linux DRM sysfs tree.
///
/// Card nodes are sorted by their numeric index so the report order is stable
/// across calls. A node whose directory cannot be read is skipped rather than
/// aborting the collection (FR-011).
fn collect_linux_adapters() -> Vec<GpuAdapter> {
    const DRM_CLASS: &str = "/sys/class/drm";

    let Ok(entries) = std::fs::read_dir(DRM_CLASS) else {
        return Vec::new();
    };

    let mut cards: Vec<(u32, std::path::PathBuf)> = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name();
            let name = name.to_str()?;
            let index = name.strip_prefix("card")?.parse::<u32>().ok()?;
            Some((index, entry.path()))
        })
        .collect();
    cards.sort_by_key(|(index, _)| *index);

    cards
        .iter()
        .map(|(_, card)| collect_linux_adapter(card))
        .collect()
}

/// Build one [`GpuAdapter`] from a DRM `cardN` sysfs node.
///
/// Reads the `device/` subtree for `vendor`, `device`, `driver`, and the
/// optional AMD `mem_info_vram_total`. Unreadable or absent values become
/// placeholders so the adapter is still reported (FR-011).
fn collect_linux_adapter(card: &std::path::Path) -> GpuAdapter {
    let device = card.join("device");

    let vendor_id = read_hex_id(&device.join("vendor"));
    let device_id = read_hex_id(&device.join("device"));
    let driver = read_driver_name(&device.join("driver"));
    let vram_bytes = read_u64(&device.join("mem_info_vram_total")).unwrap_or(0);

    let vendor = pci_vendor_name(&vendor_id).unwrap_or_else(|| UNKNOWN.to_string());
    let name = match pci_device_name(&vendor_id, &device_id) {
        Some(device_name) => format!("{vendor} {device_name}"),
        None => format!("{vendor} device {device_id}"),
    };
    let kind = classify_gpu(&driver, vram_bytes);

    GpuAdapter {
        name,
        vendor,
        vendor_id,
        device_id,
        driver,
        kind,
        vram_bytes,
    }
}

/// Read a sysfs hex id (e.g. `0x8086`) and normalise it to four lowercase hex
/// digits (e.g. `8086`); `unknown` when absent or unparseable (FR-011).
fn read_hex_id(path: &std::path::Path) -> String {
    let Some(raw) = std::fs::read_to_string(path).ok() else {
        return UNKNOWN.to_string();
    };
    let trimmed = raw.trim();
    let Some((_, digits)) = trimmed.split_once('x').or_else(|| trimmed.split_once('X')) else {
        return UNKNOWN.to_string();
    };
    match u32::from_str_radix(digits, 16) {
        Ok(value) => format!("{value:04x}"),
        Err(_) => UNKNOWN.to_string(),
    }
}

/// Resolve the kernel driver bound to a PCI device symlink (e.g. `i915`);
/// `unknown` when the symlink is absent or its target is unreadable (FR-011).
fn read_driver_name(driver_link: &std::path::Path) -> String {
    std::fs::read_link(driver_link)
        .ok()
        .and_then(|target| {
            target
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
        })
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| UNKNOWN.to_string())
}

/// Read a decimal `u64` from a sysfs file, or `None` when absent/invalid.
fn read_u64(path: &std::path::Path) -> Option<u64> {
    std::fs::read_to_string(path)
        .ok()?
        .trim()
        .parse::<u64>()
        .ok()
}

/// Classify an adapter as `integrated`, `discrete`, `virtual`, or `unknown`.
///
/// Virtual adapters are detected by well-known driver names; discrete adapters
/// are inferred from a driver hint (`nvidia`, `amdgpu`) or reported VRAM; all
/// remaining known adapters are treated as integrated (the common case for
/// Intel/AMD iGPUs). Unknown drivers stay `unknown` (FR-019).
fn classify_gpu(driver: &str, vram_bytes: u64) -> String {
    let driver = driver.to_ascii_lowercase();
    if matches!(
        driver.as_str(),
        "virtio_gpu" | "vmwgfx" | "qxl" | "vboxvideo"
    ) {
        return "virtual".to_string();
    }
    if driver == "nvidia" || driver == "nvidia-drm" || driver == "amdgpu" || vram_bytes > 0 {
        return "discrete".to_string();
    }
    if matches!(
        driver.as_str(),
        "i915" | "xe" | "radeon" | "nouveau" | "mgag200" | "ast" | "imx-drm" | "msm" | "panfrost"
    ) {
        return "integrated".to_string();
    }
    UNKNOWN.to_string()
}

/// Look up a PCI vendor name from the system `pci.ids` database.
///
/// Reads `/usr/share/hwdata/pci.ids` (or the `/usr/share/misc/pci.ids`
/// fallback) read-only and returns the vendor's display name, or `None` when
/// the database or entry is unavailable (FR-011).
fn pci_vendor_name(vendor_id: &str) -> Option<String> {
    let contents = pci_ids_contents()?;
    find_pci_vendor(&contents, vendor_id)
}

/// Look up a PCI device name (e.g. `TigerLake-LP GT2 [Iris Xe Graphics]`) from
/// the system `pci.ids` database; `None` when unavailable (FR-011).
fn pci_device_name(vendor_id: &str, device_id: &str) -> Option<String> {
    let contents = pci_ids_contents()?;
    find_pci_device(&contents, vendor_id, device_id)
}

/// Read the PCI id database from its conventional Linux locations.
///
/// Read-only; returns `None` when neither candidate file is present.
fn pci_ids_contents() -> Option<String> {
    const CANDIDATES: &[&str] = &["/usr/share/hwdata/pci.ids", "/usr/share/misc/pci.ids"];
    CANDIDATES
        .iter()
        .find_map(|path| std::fs::read_to_string(path).ok())
}

/// Extract a vendor's display name from `pci.ids` contents.
///
/// Vendor lines are unindented `vid  name` pairs; matches the four-hex-digit id
/// case-insensitively and returns the trimmed name.
fn find_pci_vendor(contents: &str, vendor_id: &str) -> Option<String> {
    contents.lines().find_map(|line| {
        if line.starts_with('\t') || line.starts_with('#') || line.trim().is_empty() {
            return None;
        }
        let (vid, name) = split_hex_name(line)?;
        (vid == vendor_id).then_some(name)
    })
}

/// Extract a device's display name from `pci.ids` contents for a vendor.
///
/// Scans from the vendor line (unindented) to the next vendor line, matching
/// single-tab device lines (`did  name`) case-insensitively.
fn find_pci_device(contents: &str, vendor_id: &str, device_id: &str) -> Option<String> {
    let mut in_vendor = false;
    for line in contents.lines() {
        if !line.starts_with('\t') {
            if line.starts_with('#') || line.trim().is_empty() {
                continue;
            }
            in_vendor = split_hex_name(line).is_some_and(|(vid, _)| vid == vendor_id);
            continue;
        }
        if !in_vendor || line.starts_with("\t\t") {
            continue;
        }
        if let Some((did, name)) = split_hex_name(line.trim_start_matches('\t'))
            && did == device_id
        {
            return Some(name);
        }
    }
    None
}

/// Split a `pci.ids` entry line into its lowercase hex id and display name.
///
/// The id is the leading hex token; the name is the remainder, trimmed. Returns
/// `None` when the line does not begin with a hex id.
fn split_hex_name(line: &str) -> Option<(String, String)> {
    let trimmed = line.trim();
    let mut parts = trimmed.split_whitespace();
    let id = parts.next()?;
    if !id.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let name = parts.collect::<Vec<_>>().join(" ");
    if name.is_empty() {
        return None;
    }
    Some((id.to_ascii_lowercase(), name))
}

impl HardwareSection {
    /// Collect the physical-hardware identity, storage, and network report
    /// (FR-022, FR-023, FR-024, FR-011).
    ///
    /// System/chassis/motherboard/firmware fields come from the read-only Linux
    /// DMI sysfs tree (`/sys/class/dmi/id`); every other platform yields the
    /// `unknown` placeholder. The collectors never fail: an unreadable value
    /// becomes `unknown` and an unreadable device tree becomes an empty list
    /// (FR-011). Serial numbers, UUIDs, and asset tags are never read (FR-025).
    fn collect() -> Self {
        Self {
            system_vendor: dmi_value("sys_vendor"),
            system_model: dmi_value("product_name"),
            system_version: dmi_value("product_version"),
            product_family: dmi_value("product_family"),
            chassis_type: dmi_value("chassis_type").trim().parse::<u32>().map_or_else(
                |_| UNKNOWN.to_string(),
                |code| chassis_type_label(code).to_string(),
            ),
            chassis_vendor: dmi_value("chassis_vendor"),
            board_vendor: dmi_value("board_vendor"),
            board_name: dmi_value("board_name"),
            board_version: dmi_value("board_version"),
            bios_vendor: dmi_value("bios_vendor"),
            bios_version: dmi_value("bios_version"),
            bios_date: dmi_value("bios_date"),
            storage: collect_storage_devices(),
            network: collect_network_interfaces(),
        }
    }
}

/// Read one value from the read-only Linux DMI sysfs tree.
///
/// Returns the trimmed file contents from `/sys/class/dmi/id/<name>`, or the
/// `unknown` placeholder when the file is absent, unreadable, or blank. Serial,
/// UUID, and asset-tag files are intentionally never requested (FR-025).
fn dmi_value(name: &str) -> String {
    let path = std::path::Path::new("/sys/class/dmi/id").join(name);
    match std::fs::read_to_string(path) {
        Ok(contents) => fallback(Some(contents.trim().to_string())),
        Err(_) => UNKNOWN.to_string(),
    }
}

/// Map a DMI chassis-type code to its SMBIOS display label (FR-022).
///
/// Codes come from the SMBIOS 3.x "System Enclosure or Chassis Types" table.
/// An unrecognised code yields the `unknown` placeholder so the field never
/// carries a bare number.
fn chassis_type_label(code: u32) -> &'static str {
    match code {
        1 => "Other",
        2 => "Unknown",
        3 => "Desktop",
        4 => "Low Profile Desktop",
        5 => "Pizza Box",
        6 => "Mini Tower",
        7 => "Tower",
        8 => "Portable",
        9 => "Laptop",
        10 => "Notebook",
        11 => "Hand Held",
        12 => "Docking Station",
        13 => "All In One",
        14 => "Sub Notebook",
        15 => "Space-saving",
        16 => "Lunch Box",
        17 => "Main Server Chassis",
        18 => "Expansion Chassis",
        19 => "Sub Chassis",
        20 => "Bus Expansion Chassis",
        21 => "Peripheral Chassis",
        22 => "RAID Chassis",
        23 => "Rack Mount Chassis",
        24 => "Sealed-case PC",
        25 => "Multi-system",
        26 => "CompactPCI",
        27 => "AdvancedTCA",
        28 => "Blade",
        29 => "Blade Enclosure",
        30 => "Tablet",
        31 => "Convertible",
        32 => "Detachable",
        33 => "IoT Gateway",
        34 => "Embedded PC",
        35 => "Mini PC",
        36 => "Stick PC",
        _ => UNKNOWN,
    }
}

/// Enumerate the host's physical storage devices from read-only sysfs (FR-023).
///
/// Reads `/sys/block` (the whole-disk view, not `/sys/class/block`, so partition
/// nodes are excluded) and skips virtual devices (`ram*`, `zram*`, `loop*`,
/// `dm-*`, `md*`, `sr*`). Devices are sorted by kernel name so the list is
/// stable across calls; an unreadable tree yields an empty list (FR-011).
fn collect_storage_devices() -> Vec<StorageDevice> {
    let Ok(entries) = std::fs::read_dir("/sys/block") else {
        return Vec::new();
    };

    let mut devices: Vec<StorageDevice> = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_str()?.to_string();
            if is_virtual_block_device(&name) {
                return None;
            }
            Some(collect_storage_device(&entry.path(), name))
        })
        .collect();
    devices.sort_by(|a, b| a.name.cmp(&b.name));
    devices
}

/// True for block-device names that are virtual rather than physical disks.
fn is_virtual_block_device(name: &str) -> bool {
    name.starts_with("ram")
        || name.starts_with("zram")
        || name.starts_with("loop")
        || name.starts_with("dm-")
        || name.starts_with("md")
        || name.starts_with("sr")
}

/// Build one [`StorageDevice`] from a `/sys/block/<name>` node (FR-023).
///
/// Reads the `device/` subtree for `vendor` and `model`, the node's `size`
/// (512-byte sectors), and the `queue/rotational` hint. Unreadable values become
/// placeholders so the device is still reported (FR-011).
fn collect_storage_device(node: &std::path::Path, name: String) -> StorageDevice {
    let device = node.join("device");
    let vendor = sysfs_trimmed(&device.join("vendor")).unwrap_or_else(|| UNKNOWN.to_string());
    let model = sysfs_trimmed(&device.join("model")).unwrap_or_else(|| UNKNOWN.to_string());
    let size_bytes = read_u64(&node.join("size")).map_or(0, |sectors| sectors * 512);
    let kind = classify_storage(&name, read_u64(&node.join("queue/rotational")));
    StorageDevice {
        name,
        model,
        vendor,
        size_bytes,
        kind,
    }
}

/// Classify a storage device by kernel-name prefix, model, and the rotational
/// hint.
///
/// NVMe devices are reported as `nvme`. A `rotational` value of `1` is `hdd` and
/// `0` is `ssd`. A missing hint is treated as `unknown` rather than guessed:
/// Linux exposes `queue/rotational` for essentially every modern block device,
/// and a USB bridge that hides it gives no reliable signal to infer from, so
/// guessing would risk mislabelling a spinner as an SSD (FR-023).
fn classify_storage(name: &str, rotational: Option<u64>) -> String {
    if name.starts_with("nvme") {
        return "nvme".to_string();
    }
    match rotational {
        Some(1) => "hdd".to_string(),
        Some(0) => "ssd".to_string(),
        _ => UNKNOWN.to_string(),
    }
}

/// Enumerate the host's network interfaces from read-only sysfs (FR-024).
///
/// Reads `/sys/class/net` and sorts by interface name so the list is stable
/// across calls; an unreadable tree yields an empty list (FR-011).
fn collect_network_interfaces() -> Vec<NetworkInterface> {
    let Ok(entries) = std::fs::read_dir("/sys/class/net") else {
        return Vec::new();
    };

    let mut interfaces: Vec<NetworkInterface> = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_str()?.to_string();
            Some(collect_network_interface(&entry.path(), name))
        })
        .collect();
    interfaces.sort_by(|a, b| a.name.cmp(&b.name));
    interfaces
}

/// Build one [`NetworkInterface`] from a `/sys/class/net/<name>` node (FR-024).
///
/// Reads `address` (MAC), `operstate`, and `speed`; a negative or unreadable
/// speed becomes `0` and an unreadable value becomes the `unknown` placeholder
/// (FR-011).
fn collect_network_interface(node: &std::path::Path, name: String) -> NetworkInterface {
    let mac_address = sysfs_trimmed(&node.join("address")).unwrap_or_else(|| UNKNOWN.to_string());
    let oper_state = sysfs_trimmed(&node.join("operstate")).unwrap_or_else(|| UNKNOWN.to_string());
    let speed_mbps = read_u64(&node.join("speed"))
        .and_then(|speed| u32::try_from(speed).ok())
        .unwrap_or(0);
    let is_loopback = name == "lo";
    NetworkInterface {
        name,
        mac_address,
        oper_state,
        speed_mbps,
        is_loopback,
    }
}

/// Read a sysfs text value, trimmed, or `None` when absent/blank (FR-011).
fn sysfs_trimmed(path: &std::path::Path) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

impl MemorySection {
    /// Collect the memory, swap, and uptime report (FR-006, FR-011).
    ///
    /// Byte counts are exact; the text renderer adds GiB and
    /// days/hours/minutes renderings. A host that cannot report boot time
    /// yields `unknown` rather than failing the call (FR-011).
    fn collect() -> Self {
        let mut system = sysinfo::System::new();
        system.refresh_memory();

        let boot_time_utc = u64_to_rfc3339(sysinfo::System::boot_time());

        Self {
            total_memory_bytes: system.total_memory(),
            available_memory_bytes: system.available_memory(),
            total_swap_bytes: system.total_swap(),
            free_swap_bytes: system.free_swap(),
            uptime_seconds: sysinfo::System::uptime(),
            boot_time_utc,
        }
    }
}

impl ProcessSection {
    /// Collect the ragent process environment report (FR-007, FR-011).
    ///
    /// Never fails: the pid and working directory come from the standard
    /// library, the shell from `$SHELL` (Unix) or `COMSPEC` (Windows), and the
    /// username from `$USER` / `$USERNAME` / `$LOGNAME`. Any value the host does
    /// not expose falls back to `unknown` (FR-011). No other environment
    /// variable is read, so no secret material can leak (FR-008).
    fn collect() -> Self {
        Self {
            pid: std::process::id(),
            working_directory: std::env::current_dir()
                .map_or_else(|_| UNKNOWN.to_string(), |dir| dir.display().to_string()),
            shell: resolve_shell(),
            username: resolve_username(),
        }
    }
}

/// Resolve the user shell: `$SHELL` on Unix, `COMSPEC` on Windows.
///
/// Returns `unknown` when the variable is unset or blank (FR-007).
fn resolve_shell() -> String {
    let key = if cfg!(windows) { "COMSPEC" } else { "SHELL" };
    fallback(std::env::var(key).ok())
}

/// Resolve the username running ragent.
///
/// Checks `USER` (Unix), `USERNAME` (Windows), then `LOGNAME`; returns
/// `unknown` when none is set (FR-007).
fn resolve_username() -> String {
    ["USER", "USERNAME", "LOGNAME"]
        .iter()
        .find_map(|key| non_empty(std::env::var(key).ok()))
        .unwrap_or_else(|| UNKNOWN.to_string())
}
