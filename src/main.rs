//! Adreno GPU Info - Basierend auf empirischen Tests
//! Getestet und funktioniert auf Adreno 610

use std::fs::File;
use std::os::unix::io::AsRawFd;
use std::mem::size_of;

// ============================================================================
// IOCTL Definitionen - Basierend auf deinen Tests
// ============================================================================

/// IOCTL Request Struktur
#[repr(C)]
struct KgslDeviceGetProperty {
    type_: u32,
    value: *mut std::ffi::c_void,
    sizebytes: u32,
    _pad: [u32; 2],
}

/// GPU Info Struktur (16 Bytes)
#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct KgslDeviceInfo {
    pub device_id: u32,      // Offset 0
    pub chip_id: u32,        // Offset 4
    pub mmu_enabled: u32,    // Offset 8
    pub gmem_gpubaseaddr: u32, // Offset 12
}

/// Version Info Struktur (8 Bytes)
#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct KgslVersionInfo {
    pub driver_version: u32,
    pub device_version: u32,
}

/// Property Types (aus msm_kgsl.h)
const KGSL_PROP_DEVICE_INFO: u32 = 0x00000001;
const KGSL_PROP_VERSION: u32 = 0x00000008;

// ============================================================================
// Chip ID Decoding
// ============================================================================

#[derive(Debug, Clone)]
struct ChipInfo {
    pub raw_id: u32,
    pub major: u8,
    pub minor: u8,
    pub patch: u8,
    pub revision: u8,
    pub model_name: String,
    pub adreno_generation: String,
    pub snapdragon_model: Option<String>,
}

fn decode_chip_id(chip_id: u32) -> ChipInfo {
    let major = ((chip_id >> 24) & 0xFF) as u8;
    let minor = ((chip_id >> 16) & 0xFF) as u8;
    let patch = ((chip_id >> 8) & 0xFF) as u8;
    let revision = (chip_id & 0xFF) as u8;

    // Bestimme Adreno Generation
    let adreno_gen = match major {
        1 => "100",
        2 => "200",
        3 => "300",
        4 => "400",
        5 => "500",
        6 => "600",
        7 => "700",
        8 => "800",
        9 => "900",
        _ => "Unknown",
    };

    // Spezifisches Modell
    let model_name = match (major, minor) {
        (6, 0) => "Adreno 600",
        (6, 1) => "Adreno 610",
        (6, 2) => "Adreno 620",
        (6, 3) => "Adreno 630",
        (6, 4) => "Adreno 640",
        (6, 5) => "Adreno 650",
        (6, 6) => "Adreno 660",
        (6, 8) => "Adreno 680",
        (6, 9) => "Adreno 690",
        (7, 0) => "Adreno 700",
        (7, 1) => "Adreno 710",
        (7, 2) => "Adreno 720",
        (7, 3) => "Adreno 730",
        (7, 4) => "Adreno 740",
        (7, 5) => "Adreno 750",
        _ => "Adreno GPU",
    };

    // Typische Snapdragon Zuordnung
    let snapdragon_model = match (major, minor) {
        (6, 1) => Some("Snapdragon 665/680/685/690/6 Gen 1"),
        (6, 2) => Some("Snapdragon 730/732G"),
        (6, 3) => Some("Snapdragon 835/845"),
        (6, 4) => Some("Snapdragon 855"),
        (6, 5) => Some("Snapdragon 865/870"),
        (6, 6) => Some("Snapdragon 888"),
        (6, 8) => Some("Snapdragon 8 Gen 1"),
        (6, 9) => Some("Snapdragon 7+ Gen 2"),
        (7, 2) => Some("Snapdragon 7 Gen 1"),
        (7, 3) => Some("Snapdragon 8+ Gen 1"),
        (7, 5) => Some("Snapdragon 8 Gen 2"),
        _ => None,
    };

    ChipInfo {
        raw_id: chip_id,
        major,
        minor,
        patch,
        revision,
        model_name: model_name.to_string(),
        adreno_generation: adreno_gen.to_string(),
        snapdragon_model: snapdragon_model.map(|s| s.to_string()),
    }
}

// ============================================================================
// Einfache, funktionierende Funktionen
// ============================================================================

/// Liest GPU Info mit der bewährten Methode
fn read_gpu_info(fd: i32) -> Result<KgslDeviceInfo, String> {
    let mut device_info = KgslDeviceInfo {
        device_id: 0,
        chip_id: 0,
        mmu_enabled: 0,
        gmem_gpubaseaddr: 0,
    };

    let mut prop = KgslDeviceGetProperty {
        type_: KGSL_PROP_DEVICE_INFO,
        value: &mut device_info as *mut _ as *mut std::ffi::c_void,
        sizebytes: size_of::<KgslDeviceInfo>() as u32,
        _pad: [0; 2],
    };

    // DIE FUNKTIONIERENDE IOCTL-NUMMER
    let ioctl_num: u32 = 0xc0140902;

    unsafe {
        let result = libc::ioctl(fd, ioctl_num as i32, &mut prop);
        if result < 0 {
            return Err(format!("IOCTL failed: {}", std::io::Error::last_os_error()));
        }
    }

    // Validiere die Daten
    if device_info.chip_id == 0 && device_info.device_id == 0 {
        return Err("Keine gültigen GPU-Daten empfangen".to_string());
    }

    Ok(device_info)
}

/// Liest die Treiberversion - KORRIGIERTE VERSION
fn read_gpu_version(fd: i32) -> Result<KgslVersionInfo, String> {
    let mut version_info = KgslVersionInfo {
        driver_version: 0,
        device_version: 0,
    };

    let mut prop = KgslDeviceGetProperty {
        type_: KGSL_PROP_VERSION,
        value: &mut version_info as *mut _ as *mut std::ffi::c_void,
        sizebytes: size_of::<KgslVersionInfo>() as u32,
        _pad: [0; 2],
    };

    // WICHTIG: Für Version brauchen wir möglicherweise eine andere IOCTL-Nummer!
    // Versuche verschiedene Kombinationen
    let possible_ioctls: [u32; 3] = [
        0xc0080902,  // 8 Bytes (wahrscheinlich richtig)
        0xc0140902,  // 20 Bytes (wie für device info)
        0xc00c0902,  // 12 Bytes
    ];

    for &ioctl_num in &possible_ioctls {
        unsafe {
            let result = libc::ioctl(fd, ioctl_num as i32, &mut prop);
            if result == 0 && (version_info.driver_version != 0 || version_info.device_version != 0) {
                return Ok(version_info);
            }
        }
    }

    Err("Version property nicht verfügbar oder benötigt andere IOCTL".to_string())
}

/// Findet KGSL-Geräte
fn find_kgsl_devices() -> Vec<String> {
    let possible_paths = [
        "/dev/kgsl-3d0",
        "/dev/kgsl/kgsl-3d0",
        "/dev/kgsl-3d1",
        "/dev/kgsl-2d0",
        "/dev/kgsl-2d1",
    ];

    possible_paths.iter()
        .filter(|path| std::path::Path::new(path).exists())
        .map(|&s| s.to_string())
        .collect()
}

// ============================================================================
// Performance/Clock Info (optional, falls verfügbar)
// ============================================================================

/// Versucht, GPU Frequenz-Informationen zu lesen
fn try_read_gpu_frequency(fd: i32) -> Option<u32> {
    // Property für GPU Frequency (kann variieren)
    const KGSL_PROP_PWRCTRL: u32 = 0x0000000E;

    let mut freq_value: u32 = 0;

    let mut prop = KgslDeviceGetProperty {
        type_: KGSL_PROP_PWRCTRL,
        value: &mut freq_value as *mut _ as *mut std::ffi::c_void,
        sizebytes: size_of::<u32>() as u32,
        _pad: [0; 2],
    };

    // Versuche verschiedene IOCTLs
    let possible_ioctls: [u32; 3] = [0xc0040902, 0xc0080902, 0xc0140902];

    for &ioctl_num in &possible_ioctls {
        unsafe {
            if libc::ioctl(fd, ioctl_num as i32, &mut prop) == 0 && freq_value != 0 {
                return Some(freq_value);
            }
        }
    }

    None
}

// ============================================================================
// Ausgabe-Funktionen
// ============================================================================

fn print_gpu_info(info: &KgslDeviceInfo, version_info: Option<&KgslVersionInfo>, freq: Option<u32>) {
    let chip_info = decode_chip_id(info.chip_id);

    println!("╔══════════════════════════════════════════════════════╗");
    println!("║                 ADRENO GPU INFORMATION               ║");
    println!("╠══════════════════════════════════════════════════════╣");
    println!("║  📱 Device: {}", chip_info.model_name);

    if let Some(snapdragon) = &chip_info.snapdragon_model {
        println!("║     Typically found in: {}", snapdragon);
    }

    println!("║  🏷️  Chip ID: 0x{:08x} (v{}.{}.{}.{})",
        chip_info.raw_id,
        chip_info.major,
        chip_info.minor,
        chip_info.patch,
        chip_info.revision
    );
    println!("║  🔢 Device ID: 0x{:08x}", info.device_id);
    println!("║  🛡️  MMU: {}", if info.mmu_enabled != 0 { "✅ Enabled" } else { "❌ Disabled" });
    println!("║  💾 GMEM Base: 0x{:08x}", info.gmem_gpubaseaddr);
    println!("║  🎯 Generation: Adreno {}", chip_info.adreno_generation);

    if let Some(freq_mhz) = freq {
        println!("║  ⚡ Frequency: {} MHz", freq_mhz / 1000000);
    }

    if let Some(ver) = version_info {
        println!("║  📊 Driver: 0x{:08x} | Device: 0x{:08x}",
            ver.driver_version, ver.device_version);
    }

    println!("║  📏 Structure: {} bytes", size_of::<KgslDeviceInfo>());

    // Raw bytes für Entwickler
    println!("╠══════════════════════════════════════════════════════╣");
    print!("║  Raw Bytes: ");
    let bytes = unsafe {
        std::slice::from_raw_parts(
            info as *const _ as *const u8,
            size_of::<KgslDeviceInfo>()
        )
    };
    for (i, byte) in bytes.iter().enumerate() {
        if i > 0 && i % 4 == 0 { print!(" "); }
        print!("{:02x}", byte);
    }
    println!();

    println!("╚══════════════════════════════════════════════════════╝");
}

// ============================================================================
// Hauptprogramm
// ============================================================================

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔍 Adreno GPU Info Tool v1.0");
    println!("   Based on empirical IOCTL testing\n");

    // Gerät finden
    let devices = find_kgsl_devices();
    if devices.is_empty() {
        eprintln!("❌ No KGSL devices found!");
        return Ok(());
    }

    println!("✅ Found {} device(s):", devices.len());
    for device in &devices {
        println!("   • {}", device);
    }
    println!();

    // Erstes Gerät öffnen
    let device_path = &devices[0];
    let file = match File::open(device_path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("❌ Cannot open {}: {}", device_path, e);
            eprintln!("   Try with root: sudo ./adreno_ioctl");
            return Ok(());
        }
    };

    let fd = file.as_raw_fd();

    // GPU Info lesen
    match read_gpu_info(fd) {
        Ok(info) => {
            // Version-Info (optional)
            let version_info = read_gpu_version(fd).ok();

            // Frequency-Info (optional)
            let freq_info = try_read_gpu_frequency(fd);

            // Alles ausgeben
            print_gpu_info(&info, version_info.as_ref(), freq_info);

            // Zusätzliche Info
            println!("\n💡 IOCTL Information:");
            println!("   • Working IOCTL: 0xc0140902");
            println!("   • Command: 0x02 (KGSL_IOC_GETPROPERTY)");
            println!("   • Type: 0x09 (KGSL_IOC_TYPE)");
            println!("   • Size: 20 bytes (returns 16 bytes)");
            println!("   • Direction: IOWR (Read/Write)");

            // Export für andere Projekte
            println!("\n📋 For use in other projects:");
            println!("   struct KgslDeviceInfo {{");
            println!("       device_id: u32,      // offset 0");
            println!("       chip_id: u32,        // offset 4");
            println!("       mmu_enabled: u32,    // offset 8");
            println!("       gmem_gpubaseaddr: u32, // offset 12");
            println!("   }}");

        }
        Err(e) => {
            eprintln!("❌ Error: {}", e);
            eprintln!("\n🔧 Troubleshooting:");
            eprintln!("   1. Run as root: sudo ./adreno_ioctl");
            eprintln!("   2. Check permissions: ls -la /dev/kgsl*");
            eprintln!("   3. Alternative IOCTLs to try:");
            eprintln!("      • 0xc0100902 (16 bytes)");
            eprintln!("      • 0xc0080902 (8 bytes)");
            eprintln!("      • 0xc00c0902 (12 bytes)");
        }
    }

    Ok(())
}
