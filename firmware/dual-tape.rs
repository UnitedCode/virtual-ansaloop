//! Proposed Rev5 carrier firmware: real GPIO inputs, TIM2 and stereo SAI IRQ.
#![no_std]
#![no_main]
#[path = "control.rs"]
mod control;
mod digital;
mod sequencer;
use control::{Command, Controller};
use core::{
    arch::{asm, global_asm},
    ptr::{read_volatile, write_volatile},
};
unsafe fn read(a: u32) -> u32 {
    read_volatile(a as *const u32)
}
unsafe fn write(a: u32, v: u32) {
    write_volatile(a as *mut u32, v);
}
static mut CONTROL: [Controller; 2] = [Controller::new(), Controller::new()];
static mut PREVIOUS: [u32; 2] = [0; 2];
static mut TICKS: u32 = 0;
static mut GATES: [u16; 2] = [0; 2];
static mut SEQ: sequencer::Sequencer = sequencer::Sequencer::new();
static mut CUE_HELD: bool = false;
static mut DIGITAL: digital::Looper = digital::Looper::new();
static mut DIGITAL_MODE: bool = false;
global_asm!(
    r#"
.syntax unified
.thumb
.section .vectors,"a",%progbits
.word 0x2001f000,tape_main
.rept 101
.word unexpected
.endr
.word audio_irq
"#
);
unsafe fn buttons(deck: usize) -> u32 {
    if deck == 0 {
        ((read(0x58020810) >> 10) & 3) | (((read(0x58020410) >> 12) & 1) << 2)
    } else {
        ((read(0x58020810) >> 4) & 3) | (((read(0x58020410) >> 13) & 1) << 2)
    }
}
#[no_mangle]
pub unsafe extern "C" fn audio_irq() {
    let seq = &mut *core::ptr::addr_of_mut!(SEQ);
    let digital = &mut *core::ptr::addr_of_mut!(DIGITAL);
    for _ in 0..16 {
        if read(0x4001101c) & 32 == 0 {
            break;
        }
        seq.receive(read(0x40011024) as u8);
    }
    if let Some((command, length)) = seq.digital_request.take() {
        if command == 6 {
            digital.clear();
        } else {
            digital.restore(length);
        }
    }
    for _ in 0..2 {
        if read(0x4001101c) & 128 == 0 {
            break;
        }
        if let Some(b) = seq.pop() {
            write(0x40011028, u32::from(b));
        } else {
            break;
        }
    }
    if TICKS & 15 == 0 {
        let c = read(0x58020810);
        let selector = read(0x58020410);
        let sequence_mode = selector & (1 << 14) != 0;
        let digital_mode = c & (1 << 13) != 0;
        if DIGITAL_MODE && !digital_mode {
            digital.stop();
        }
        DIGITAL_MODE = digital_mode;
        seq.track = usize::from(selector & (1 << 15) != 0);
        let cue = c & (1 << 12) != 0;
        if cue && !CUE_HELD && sequence_mode {
            seq.cue();
        } else if cue && !CUE_HELD && digital_mode {
            digital.position = 0;
        }
        CUE_HELD = cue;
        for deck in 0..2 {
            let b = buttons(deck);
            let rising = b & !PREVIOUS[deck];
            PREVIOUS[deck] = b;
            if sequence_mode {
                if rising & 4 != 0 {
                    seq.stop(true);
                } else if rising & 2 != 0 {
                    seq.toggle();
                } else if rising & 1 != 0 {
                    seq.start(!seq.external);
                }
            } else if digital_mode {
                if rising & 4 != 0 {
                    digital.stop();
                } else if rising & 2 != 0 {
                    digital.record();
                } else if rising & 1 != 0 {
                    digital.play();
                }
            }
            let request = if digital_mode {
                Some(Command::Stop)
            } else if sequence_mode {
                None
            } else if rising & 4 != 0 {
                Some(Command::Stop)
            } else if rising & 2 != 0 {
                Some(Command::Record)
            } else if rising & 1 != 0 {
                Some(Command::Play)
            } else {
                None
            };
            let media_pin = if deck == 0 { 8 } else { 6 };
            let ready_pin = if deck == 0 { 9 } else { 7 };
            GATES[deck] = (*core::ptr::addr_of_mut!(CONTROL))[deck].tick(
                read(0x40000024),
                request,
                c & (1 << media_pin) != 0,
                c & (1 << ready_pin) != 0,
            );
        }
        let gates = GATES[0] | (GATES[1] << 4);
        write(
            0x58020018,
            u32::from(gates) | (u32::from(!gates & 255) << 16),
        );
    }
    TICKS = TICKS.wrapping_add(1);
    // Full eight-word FIFO: four stereo frames per interrupt.
    if read(0x40015818) & 8 != 0 {
        for _ in 0..4 {
            seq.sample();
            let left = read(0x40015820);
            let right = read(0x40015820);
            if DIGITAL_MODE {
                let sample = digital.sample(left);
                write(0x40015840, sample);
                write(0x40015840, sample);
            } else {
                write(0x40015840, if GATES[0] & 3 == 3 { left } else { 0 });
                write(0x40015840, if GATES[1] & 3 == 3 { right } else { 0 });
            }
        }
    }
    // Proposed post-codec analog gates. Record-head DAC signals stay untouched.
    let gates = u32::from(seq.gates) << 8;
    write(0x58020018, gates | ((!gates & 0x300) << 16));
    let digital_flags =
        (u32::from(DIGITAL_MODE) << 10) | (u32::from(DIGITAL_MODE && digital.capture()) << 11);
    write(0x58020018, digital_flags | ((!digital_flags & 0xc00) << 16));
    // Read-only host diagnostics at the control rate, not per audio frame.
    if TICKS & 15 == 1 {
        for (i, v) in [
            u32::from(DIGITAL_MODE),
            digital.state,
            digital.length,
            digital.position,
            digital.revision,
            digital.wraps,
            digital::CAPACITY,
        ]
        .into_iter()
        .enumerate()
        {
            write(0x20000100 + i as u32 * 4, v);
        }
        for (i, v) in [
            u32::from(seq.running),
            u32::from(seq.step),
            u32::from(seq.cursor),
            u32::from(seq.bpm),
            u32::from(seq.masks[0]),
            u32::from(seq.masks[1]),
            u32::from(seq.target),
            u32::from(seq.external),
            seq.track as u32,
            seq.overflow,
        ]
        .into_iter()
        .enumerate()
        {
            write(0x20000020 + i as u32 * 4, v);
        }
        for t in 0..2 {
            for s in 0..16 {
                write(
                    0x20000060 + (t * 16 + s) as u32 * 4,
                    u32::from(seq.notes[t][s]),
                );
            }
        }
    }
}
unsafe fn codec(reg: u32, value: u32) {
    write(0x40005804, (0x1a << 1) | (2 << 16) | (1 << 25) | (1 << 13));
    write(0x40005828, (reg << 1) | (value >> 8));
    write(0x40005828, value & 255);
}
#[no_mangle]
pub unsafe extern "C" fn tape_main() -> ! {
    CONTROL = [Controller::new(), Controller::new()];
    TICKS = 0;
    GATES = [0; 2];
    SEQ = sequencer::Sequencer::new();
    CUE_HELD = false;
    DIGITAL = digital::Looper::new();
    DIGITAL_MODE = false;
    write(0x20000000, 0);
    write(0x20000004, 0);
    write(0x580244e0, 7); // GPIO A/B/C clocks.
    write(0x58020018, 255 << 16);
    write(0x58020000, (read(0x58020000) & !0xffffff) | 0x555555);
    write(0x58020400, read(0x58020400) & !(255 << 24));
    write(0x58020800, read(0x58020800) & !(0xfffff << 8));
    PREVIOUS = [buttons(0), buttons(1)]; // Held record at boot is not a new request.
    write(0x580244e8, (1 << 22) | 1); // I2C2 and TIM2.
    write(0x40000028, 63);
    write(0x4000002c, u32::MAX);
    write(0x40000014, 1);
    write(0x40000000, 1);
    write(0x40005800, 1);
    for (reg, value) in [
        (0, 23),
        (1, 23),
        (4, 0x12),
        (5, 0),
        (6, 0x42),
        (7, 9),
        (8, 0),
        (9, 1),
    ] {
        codec(reg, value);
    }
    write(0x580244f0, (1 << 22) | (1 << 4));
    write(0x4001100c, 2048); // HSI64 / 2048 = MIDI 31,250 baud.
    write(0x40011000, 13); // UE, RE, TE; poll without an extra IRQ.
    write(0x58024450, 4); // HSI64 -> MCLK16MHz -> 62500 frames/s.
    for channel in 0..2 {
        let base = 0x40015800 + channel * 32;
        write(base + 0xc, 63 | (31 << 8) | (1 << 16) | (1 << 18));
        write(base + 0x10, 0x30180);
        write(base + 8, if channel == 0 { 4 } else { 0 });
        write(
            base + 4,
            (4 << 20) | (6 << 5) | if channel == 0 { 1 } else { 2 | (1 << 10) },
        );
    }
    for _ in 0..8 {
        write(0x40015840, 0);
    }
    write(0x40015814, 8); // RX FIFO request IRQ.
    write(0xe000e108, 1 << 23); // NVIC IRQ87.
    write(0x40015824, read(0x40015824) | (1 << 16));
    write(0x40015804, read(0x40015804) | (1 << 16));
    write(0x20000000, 0xa115a);
    asm!("cpsie i");
    loop {
        asm!("wfi");
    }
}
#[no_mangle]
pub unsafe extern "C" fn unexpected() -> ! {
    write(0x20000004, 0xbad);
    loop {
        asm!("wfi");
    }
}
#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    unsafe { unexpected() }
}
