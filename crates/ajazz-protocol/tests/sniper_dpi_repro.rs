use ajazz_protocol::mouse::{build_set_keymap_packet, MouseButtonAction, MouseButtonSlot};
use ajazz_protocol::SystemKey;

/// Regressão permanente (diagnosing-bugs Fase 5):
/// `0x40 01/02/04` são atalhos ACPI System Control (`sub_4247b0` + `sub_415a80`: Power/Sleep/Wake),
/// nunca "Sniper DPI". Ações de DPI por botão devem emitir apenas `1F 01/00 00 00`.
#[test]
fn system_power_opcodes_are_explicit_and_dpi_buttons_use_1f() {
    let mut slots = MouseButtonSlot::DEFAULT_FIVE;
    slots[2] = MouseButtonSlot::Mouse(MouseButtonAction::DpiMinus);
    let dpi_pkt = build_set_keymap_packet(&slots);
    assert_eq!(&dpi_pkt[13..17], &[0x1F, 0x00, 0x00, 0x00]);
    assert_ne!(dpi_pkt[13], 0x40);

    slots[2] = MouseButtonSlot::Mouse(MouseButtonAction::DpiPlus);
    let dpi_plus_pkt = build_set_keymap_packet(&slots);
    assert_eq!(&dpi_plus_pkt[13..17], &[0x1F, 0x01, 0x00, 0x00]);

    assert_eq!(
        MouseButtonSlot::System(SystemKey::Power).to_wire(),
        [0x40, 0x01, 0x00, 0x00]
    );
    assert_eq!(
        MouseButtonSlot::System(SystemKey::Sleep).to_wire(),
        [0x40, 0x02, 0x00, 0x00]
    );
    assert_eq!(
        MouseButtonSlot::System(SystemKey::Wake).to_wire(),
        [0x40, 0x04, 0x00, 0x00]
    );
}
