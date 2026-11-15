use ajazz_protocol::macros::{MacroAction, MacroDefinition, MacroStep};
use ajazz_protocol::mouse::{
    build_set_keymap_packet, build_upload_mouse_macros_packets, compile_mouse_macros_container,
    validate_outgoing_packet, MouseButtonAction, MouseButtonSlot,
};

fn oracle_clamp_delay(delay_ms: i32) -> u16 {
    if delay_ms <= 0 {
        10
    } else {
        (delay_ms.min(0x2710)) as u16
    }
}

/// Oracle faithful to `sub_430860` (`AJAZZ Driver（W1）.exe`): takes `t_macrorecord`
/// records where `delay_time` is the post-delay AFTER event `i`.
fn oracle_compile_mouse_macros_container(macros: &[&[(u8, bool, u16)]]) -> Vec<u8> {
    let mut buf = vec![0u8; 0x40];
    let mut off = 0x40usize;

    for (slot_idx, steps) in macros.iter().take(32).enumerate() {
        if steps.is_empty() {
            continue;
        }
        buf[slot_idx * 2..slot_idx * 2 + 2].copy_from_slice(&(off as u16).to_le_bytes());
        let mut next_delay: i32 = 10;
        for (i, &(hid_usage, down, post_delay_ms)) in steps.iter().enumerate() {
            let [d_lo, d_hi] = (next_delay.clamp(0, 0x2710) as u16).to_le_bytes();
            let flags = if down { 0x42 } else { 0x02 };
            buf.resize(off + 4, 0);
            buf[off] = d_lo;
            buf[off + 1] = d_hi;
            buf[off + 2] = flags;
            buf[off + 3] = hid_usage;

            next_delay = i32::from(post_delay_ms);
            if i + 1 == steps.len() {
                let term_delay = oracle_clamp_delay(next_delay);
                let [t_lo, t_hi] = term_delay.to_le_bytes();
                buf.resize(off + 12, 0);
                buf[off + 4] = t_lo;
                buf[off + 5] = t_hi;
                buf[off + 6] = 0x83;
                buf[off + 7] = 0x01;
                off += 8;
            }
            off += 4;
        }
    }
    buf.truncate(0xFA0);
    buf
}

fn rmaon_steps(delays: [u16; 10]) -> Vec<MacroStep> {
    // R=0x15, M=0x10, A=0x04, O=0x12, N=0x11
    let keys = [0x15, 0x15, 0x10, 0x10, 0x04, 0x04, 0x12, 0x12, 0x11, 0x11];
    keys.into_iter()
        .enumerate()
        .map(|(i, hid_usage)| MacroStep {
            delay_ms: delays[i],
            action: MacroAction::Key {
                hid_usage,
                down: i % 2 == 0,
            },
        })
        .collect()
}

#[test]
fn diff_oracle_06_12_macro_10x_minimal() {
    // Minimised repro from Phase 2:
    // `sub_425440` + `sub_415790` (`label_4158b9`): `[0x60, index, loop_count, loop_mode]`.
    let slot = MouseButtonSlot::Macro {
        index: 0,
        loop_mode: 0,
        loop_count: 10,
    };
    assert_eq!(slot.to_wire(), [0x60, 0x00, 0x0A, 0x00]);

    let pkt = build_set_keymap_packet(&[slot]);
    assert_eq!(&pkt[5..9], &[0x60, 0x00, 0x0A, 0x00]);
    assert!(validate_outgoing_packet(&pkt).is_ok());

    // Modes 1 ("Até soltar") and 2 ("Alternar") carry loop_count >= 1 (`[0x60, idx, 1, mode]`).
    let hold = MouseButtonSlot::Macro {
        index: 0,
        loop_mode: 1,
        loop_count: 1,
    };
    assert_eq!(hold.to_wire(), [0x60, 0x00, 0x01, 0x01]);

    let toggle = MouseButtonSlot::Macro {
        index: 0,
        loop_mode: 2,
        loop_count: 1,
    };
    assert_eq!(toggle.to_wire(), [0x60, 0x00, 0x01, 0x02]);

    // Safety net: `loop_count = 0` or `loop_mode > 2` must never be emitted raw to MCU flash.
    let coerced = MouseButtonSlot::Macro {
        index: 0,
        loop_mode: 0,
        loop_count: 0,
    };
    assert_eq!(coerced.to_wire(), [0x60, 0x00, 0x01, 0x00]);

    let raw_bad_count = build_set_keymap_packet(&[MouseButtonSlot::Raw([0x60, 0x00, 0x00, 0x0A])]);
    assert!(validate_outgoing_packet(&raw_bad_count).is_err());

    let raw_bad_mode = build_set_keymap_packet(&[MouseButtonSlot::Raw([0x60, 0x00, 0x01, 0x0A])]);
    assert!(validate_outgoing_packet(&raw_bad_mode).is_err());
}

#[test]
fn diff_oracle_rmaon_10x_full_sequence() {
    let steps = rmaon_steps([0, 60, 45, 55, 40, 50, 48, 52, 42, 65]);
    let macro_def = MacroDefinition {
        id: "macro_1".into(),
        name: "rmaon".into(),
        steps: steps.clone(),
    };

    let rust_container = compile_mouse_macros_container(&[macro_def]).unwrap();
    // Convert Rust pre-delay representation (`step[i].delay_ms` precedes `step[i]`) to
    // AJAZZ `t_macrorecord` post-delay records (`rec[i].post_delay == step[i+1].delay_ms`).
    let oracle_records: Vec<(u8, bool, u16)> = steps
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let MacroAction::Key { hid_usage, down } = s.action else {
                unreachable!()
            };
            let post_delay = steps.get(i + 1).map_or(10, |next| next.delay_ms);
            (hid_usage, down, post_delay)
        })
        .collect();
    let oracle_container = oracle_compile_mouse_macros_container(&[&oracle_records]);
    assert_eq!(rust_container, oracle_container);

    let upload_pkts = build_upload_mouse_macros_packets(&rust_container).unwrap();
    assert!(!upload_pkts.is_empty());
    for pkt in &upload_pkts {
        assert!(validate_outgoing_packet(pkt).is_ok());
    }

    let slots = [
        MouseButtonSlot::Mouse(MouseButtonAction::Left),
        MouseButtonSlot::Mouse(MouseButtonAction::Right),
        MouseButtonSlot::Mouse(MouseButtonAction::Middle),
        MouseButtonSlot::Mouse(MouseButtonAction::Forward),
        MouseButtonSlot::Macro {
            index: 0,
            loop_mode: 0,
            loop_count: 10,
        },
    ];
    let keymap_pkt = build_set_keymap_packet(&slots);
    assert_eq!(&keymap_pkt[0..5], &[0x06, 0x12, 0x20, 0x00, 0x00]);
    assert_eq!(&keymap_pkt[21..25], &[0x60, 0x00, 0x0A, 0x00]);
    assert!(validate_outgoing_packet(&keymap_pkt).is_ok());
}
