use ajazz_protocol::MOUSE_NEARLINK_VID;
use ajazz_protocol::mouse::{
    Aj179MouseDevice, MOUSE_VENDOR_USAGE, MOUSE_VENDOR_USAGE_PAGE,
};

fn main() {
    let api = ajazz_protocol::hidapi::HidApi::new().expect("Falha ao inicializar hidapi");

    println!("Interfaces HID Safe (VID 0x{:04X}, UsagePage 0x{:04X}, Usage 0x{:02X}):",
        MOUSE_NEARLINK_VID, MOUSE_VENDOR_USAGE_PAGE, MOUSE_VENDOR_USAGE);
    for d in api.device_list() {
        if d.vendor_id() == MOUSE_NEARLINK_VID {
            println!(
                "  PID=0x{:04X} UsagePage=0x{:04X} Usage=0x{:02X} Product={:?}",
                d.product_id(),
                d.usage_page(),
                d.usage(),
                d.product_string()
            );
        }
    }

    match Aj179MouseDevice::open(&api) {
        Ok(mut mouse) => {
            println!("Conectado via {:?}", mouse.connection_mode());
            match mouse.read_receiver_fw() {
                Ok(fw) => println!("Receiver FW (06 07): {:?}", fw),
                Err(e) => println!("Receiver FW erro: {}", e),
            }
            match mouse.read_status_raw() {
                Ok(raw) => {
                    println!("Raw RX (06 05): {:02X?}", &raw[..44]);
                    match ajazz_protocol::mouse::parse_status_response(
                        &raw,
                        mouse.connection_mode(),
                        None,
                    ) {
                        Ok(st) => println!("Parsed MouseStatus: {:#?}", st),
                        Err(e) => println!("Parse erro: {}", e),
                    }
                }
                Err(e) => println!("Erro lendo status bruto: {}", e),
            }
        }
        Err(e) => println!("Mouse AJ179 não aberto: {}", e),
    }
}
