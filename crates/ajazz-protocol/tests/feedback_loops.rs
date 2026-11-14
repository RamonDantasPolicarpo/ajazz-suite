use ajazz_protocol::{
    build_custom_matrix_packets_for_bank, build_lighting_packet_config, build_live_matrix_packets,
    calculate_checksum, scale_color_by_brightness, LightingConfig, LightingDirection,
    LightingMode, RgbColor, AK820_KEY_LAYOUT, HID_REPORT_PAYLOAD_SIZE, LIVE_MATRIX_BYTES,
    OPCODE_SET_LIVE_MATRIX, TOTAL_PHYSICAL_LEDS,
};

#[test]
fn bug1_static_red_full_64_bytes_must_match_official_pcap_frame_117() {
    // Oracle: docs/captures/ak820_red.pcapng Frame 117 (software oficial sem inversão WASD)
    let official_frame_117: [u8; 64] = [
        0x04, 0x30, 0x02, 0x06, 0x1d, 0x00, 0x00, 0x00, 0x00, 0x06, 0x04, 0x04, 0x00, 0x00, 0xff,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xff, 0x00,
        0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00,
    ];

    let cfg = LightingConfig {
        mode: LightingMode::Static,
        brightness: 4,
        speed: 0,
        direction: LightingDirection::Primary,
        multicolor: false,
        color: RgbColor::new(255, 0, 0),
        custom_group: 1,
        zzcc_mode: 0,
    };

    let actual = build_lighting_packet_config(&cfg);
    assert_eq!(
        actual, official_frame_117,
        "Full 64-byte HID payload diverged from official Frame 117"
    );
}

#[test]
fn bug2_custom_bank_packets_match_official_sub_47a890_and_rctrl_override() {
    // Oracle from sub_49ef80 + sub_49f280 + sub_47a890:
    // - 126 keys * 3 = 378 bytes (0x17A) payload sent via 7 chunks (56 * 6 + 42), OR 384 bytes (56*6 + 48).
    // - Crucial: checksum in official sub_47a890 covers payload[3..64], offset = bank*0x200 + chunk_offset.
    // - Crucial: in sub_49ef80, light_id 0x70 (RCtrl, 112) overrides phys_led to 0x5D (93)!
    let rctrl = AK820_KEY_LAYOUT
        .iter()
        .find(|k| k.light_id == 112)
        .expect("RCtrl missing in layout");
    assert_eq!(
        rctrl.phys_led, 93,
        "RCtrl (light_id=112) must map to phys_led=93 (0x5D) per official sub_49ef80, got {}",
        rctrl.phys_led
    );

    // Also verify all 7 chunks of custom bank 1 have valid checksums and exact bank offset 0x0200
    let mut frame = [RgbColor::default(); TOTAL_PHYSICAL_LEDS];
    let w_key = AK820_KEY_LAYOUT.iter().find(|k| k.name == "W").unwrap();
    frame[w_key.phys_led] = RgbColor::new(0x12, 0x34, 0x56);

    let packets = build_custom_matrix_packets_for_bank(1, &frame);
    assert_eq!(packets.len(), 7);
    assert_eq!(u16::from_le_bytes([packets[0][5], packets[0][6]]), 0x0200);
    for pkt in &packets {
        let sum = calculate_checksum(pkt);
        assert_eq!(u16::from_le_bytes([pkt[1], pkt[2]]), sum);
        assert_eq!(pkt.len(), HID_REPORT_PAYLOAD_SIZE);
    }
}

#[test]
fn bug4_capture_mode_requires_volatile_0x12_stream_builder() {
    // Oracle: sub_47ab30 + sub_4b4310 envia 0x1E -> 0xFE em 0x06 e faz streaming de 378 bytes via Opcode 0x12
    let mut frame = [RgbColor::default(); TOTAL_PHYSICAL_LEDS];
    let esc = &AK820_KEY_LAYOUT[0];
    frame[esc.phys_led] = RgbColor::new(200, 100, 50);

    let packets = build_live_matrix_packets(&frame, 4);
    assert_eq!(packets.len(), 7);
    let total_bytes: usize = packets.iter().map(|p| p[4] as usize).sum();
    assert_eq!(total_bytes, LIVE_MATRIX_BYTES);
    for (i, pkt) in packets.iter().enumerate() {
        assert_eq!(pkt[3], OPCODE_SET_LIVE_MATRIX);
        assert_eq!(u16::from_le_bytes([pkt[5], pkt[6]]) as usize, i * 56);
        assert_eq!(u16::from_le_bytes([pkt[1], pkt[2]]), calculate_checksum(pkt));
    }

    let scaled = scale_color_by_brightness(RgbColor::new(200, 100, 50), 4);
    assert_eq!((packets[0][8], packets[0][9], packets[0][10]), (scaled.r, scaled.g, scaled.b));

    let dimmed = build_live_matrix_packets(&frame, 0);
    assert_eq!((dimmed[0][8], dimmed[0][9], dimmed[0][10]), (0, 0, 0));
}
