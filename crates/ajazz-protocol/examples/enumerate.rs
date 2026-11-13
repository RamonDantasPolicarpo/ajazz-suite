use ajazz_protocol::{AK820_PID, AK820_VID, MOUSE_NEARLINK_PID, MOUSE_NEARLINK_VID};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let api = hidapi::HidApi::new()?;
    println!("=== Dispositivos Reconhecidos no Sistema ===");
    for (i, d) in api.device_list().enumerate() {
        let vid = d.vendor_id();
        let pid = d.product_id();
        let is_ak820 = vid == AK820_VID && pid == AK820_PID;
        let is_mouse = vid == MOUSE_NEARLINK_VID && pid == MOUSE_NEARLINK_PID;

        if is_ak820 || is_mouse {
            let label = if is_ak820 { "TECLADO AK820" } else { "MOUSE NEARLINK" };
            println!(
                "[{:02}] [{}] VID=0x{:04X}, PID=0x{:04X}, Manuf={:?}, Prod={:?}, UsagePage=0x{:04X}, Usage=0x{:04X}",
                i, label, vid, pid, d.manufacturer_string(), d.product_string(), d.usage_page(), d.usage()
            );
        }
    }
    Ok(())
}
