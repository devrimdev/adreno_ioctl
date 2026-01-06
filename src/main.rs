//! Adreno GPU Info - Basierend auf empirischen Tests
//! Getestet und funktioniert auf Adreno 610

use std::fs::File;
use std::os::unix::io::AsRawFd;
use std::mem::size_of;

// ============================================================================
// IOCTL Definitionen - Basierend auf deinen Tests
// ============================================================================

/// IOCTL Request Struktur (genau wie in deinem Test)
#[repr(C)]
struct KgslDeviceGetProperty {
    type_: u32,
    value: *mut std::ffi::c_void,
    sizebytes: u32,
    _pad: [u32; 2],
}

/// GPU Info Struktur (16 Bytes - so wie es tatsächlich zurückkommt!)
#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct KgslDeviceInfo {
    pub device_id: u32,      // Offset 0
    pub chip_id: u32,        // Offset 4
    pub mmu_enabled: u32,    // Offset 8
    pub gmem_gpubaseaddr: u32, // Offset 12
}

/// Property Types
const KGSL_PROP_DEVICE_INFO: u32 = 0x1;
const KGSL_PROP_VERSION: u32 = 0x8;

// ============================================================================
// Chip ID Decoding - Basierend auf deinem Chip 0x06010001
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
        6 => "600",  // Dein Fall!
        7 => "700",
        8 => "800",
        9 => "900",
        _ => "Unknown",
    };

    // Spezifisches Modell (basierend auf major.minor)
    let model_name = match (major, minor) {
        (6, 0) => "Adreno 600".to_string(),
        (6, 1) => "Adreno 610".to_string(),  // DEIN GPU!
        (6, 2) => "Adreno 620".to_string(),
        (6, 3) => "Adreno 630".to_string(),
        (6, 4) => "Adreno 640".to_string(),
        (6, 5) => "Adreno 650".to_string(),
        (6, 6) => "Adreno 660".to_string(),
        (6, 8) => "Adreno 680".to_string(),
        (6, 9) => "Adreno 690".to_string(),
        (7, 0) => "Adreno 700".to_string(),
        (7, 1) => "Adreno 710".to_string(),
        (7, 2) => "Adreno 720".to_string(),
        (7, 3) => "Adreno 730".to_string(),
        (7, 4) => "Adreno 740".to_string(),
        (7, 5) => "Adreno 750".to_string(),
        _ => format!("Adreno {}{}0", major, minor),
    };

    ChipInfo {
        raw_id: chip_id,
        major,
        minor,
        patch,
        revision,
        model_name,
        adreno_generation: adreno_gen.to_string(),
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
        sizebytes: size_of::<KgslDeviceInfo>() as u32,  // 16 Bytes
        _pad: [0; 2],
    };

    // DIE FUNKTIONIERENDE IOCTL-NUMMER AUS DEINEM TEST!
    let ioctl_num: u32 = 0xc0140902;  // IOWR(3, 0x09, 0x02, 20)

    unsafe {
        let result = libc::ioctl(fd, ioctl_num as i32, &mut prop);
        if result < 0 {
            return Err(format!("IOCTL failed: {}", std::io::Error::last_os_error()));
        }
    }

    // Validiere die Daten (nicht alles 0)
    if device_info.chip_id == 0 && device_info.device_id == 0 {
        return Err("Keine gültigen GPU-Daten empfangen".to_string());
    }

    Ok(device_info)
}

/// Liest die Treiberversion
fn read_gpu_version(fd: i32) -> Result<(u32, u32), String> {
    let mut version_buf = [0u32; 2];  // [driver_version, device_version]

    let mut prop = KgslDeviceGetProperty {
        type_: KGSL_PROP_VERSION,
        value: version_buf.as_mut_ptr() as *mut std::ffi::c_void,
        sizebytes: size_of::<[u32; 2]>() as u32,
        _pad: [0; 2],
    };

    // Gleiche IOCTL-Nummer, nur type_ ist anders
    let ioctl_num: u32 = 0xc0140902;

    unsafe {
        let result = libc::ioctl(fd, ioctl_num as i32, &mut prop);
        if result < 0 {
            return Err(format!("Version IOCTL failed: {}", std::io::Error::last_os_error()));
        }
    }

    Ok((version_buf[0], version_buf[1]))
}

/// Findet KGSL-Geräte
fn find_kgsl_devices() -> Vec<String> {
    let possible_paths = [
        "/dev/kgsl-3d0",
        "/dev/kgsl/kgsl-3d0",
        "/dev/kgsl-3d1",
    ];

    possible_paths.iter()
        .filter(|path| std::path::Path::new(path).exists())
        .map(|&s| s.to_string())
        .collect()
}

// ============================================================================
// Ausgabe-Funktionen
// ============================================================================

fn print_gpu_info(info: &KgslDeviceInfo) {
    let chip_info = decode_chip_id(info.chip_id);

    println!("🎮 GPU Informationen:");
    println!("   Modell:         {}", chip_info.model_name);
    println!("   Generation:     Adreno {}", chip_info.adreno_generation);
    println!("   Chip ID:        0x{:08x}", info.chip_id);
    println!("   Version:        {}.{}.{}.{}",
        chip_info.major,
        chip_info.minor,
        chip_info.patch,
        chip_info.revision
    );
    println!("   Device ID:      0x{:08x}", info.device_id);
    println!("   MMU:            {}", if info.mmu_enabled != 0 { "Aktiviert" } else { "Deaktiviert" });
    println!("   GMEM Basis:     0x{:08x}", info.gmem_gpubaseaddr);

    // Byteweise Ausgabe für Debugging
    let bytes = unsafe {
        std::slice::from_raw_parts(
            info as *const _ as *const u8,
            size_of::<KgslDeviceInfo>()
        )
    };
    print!("   Rohdaten (16B): ");
    for byte in bytes {
        print!("{:02x} ", byte);
    }
    println!();
}

// ============================================================================
// Hauptprogramm
// ============================================================================

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔍 Adreno GPU Info Tool");
    println!("========================\n");

    // Gerät finden
    let devices = find_kgsl_devices();
    if devices.is_empty() {
        eprintln!("❌ Keine KGSL-Geräte gefunden!");
        println!("   Mögliche Ursachen:");
        println!("   1. Kein Adreno/Qualcomm GPU");
        println!("   2. Keine Root-Rechte");
        println!("   3. Kernel hat KGSL nicht aktiviert");
        return Ok(());
    }

    println!("✅ Gefundene Geräte:");
    for device in &devices {
        println!("   - {}", device);
    }
    println!();

    // Erstes Gerät öffnen
    let device_path = &devices[0];
    let file = match File::open(device_path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("❌ Konnte {} nicht öffnen: {}", device_path, e);
            eprintln!("   Versuche mit Root-Rechten: sudo ./programm");
            return Ok(());
        }
    };

    let fd = file.as_raw_fd();

    // GPU Info lesen
    match read_gpu_info(fd) {
        Ok(info) => {
            print_gpu_info(&info);

            // Version lesen
            match read_gpu_version(fd) {
                Ok((driver_ver, device_ver)) => {
                    println!("\n📊 Treiber Version:");
                    println!("   Treiber: 0x{:08x}", driver_ver);
                    println!("   Gerät:   0x{:08x}", device_ver);
                }
                Err(e) => {
                    println!("\n⚠️  Keine Version-Info: {}", e);
                }
            }

            // Kompatibilitäts-Info
            println!("\n💡 Diese IOCTL-Kombination funktioniert auf:");
            println!("   • Adreno 610 (wie getestet)");
            println!("   • Adreno 600-Serie (600, 620, 630, etc.)");
            println!("   • Wahrscheinlich auch 500, 700 Serie");
        }
        Err(e) => {
            eprintln!("❌ Fehler: {}", e);
            eprintln!("\n💡 Lösungsvorschläge:");
            eprintln!("   1. Mit Root ausführen: sudo ./programm");
            eprintln!("   2. Gerätedatei prüfen: ls -la /dev/kgsl*");
            eprintln!("   3. Kernel-Logs: dmesg | grep kgsl");
            eprintln!("   4. Teste alternative IOCTL:");
            eprintln!("      • 0xc0100902 (16 Bytes statt 20)");
            eprintln!("      • 0xc0080902 (8 Bytes)");
        }
    }

    Ok(())
}
