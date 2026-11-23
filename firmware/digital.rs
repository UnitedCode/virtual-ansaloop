//! Mono 24-bit looper. Every sample and playhead update executes on the ARM.
use core::ptr::{read_volatile, write_volatile};
pub const BUFFER: u32 = 0xc0000000;
pub const CAPACITY: u32 = 62_500 * 60;
pub struct Looper {
    pub state: u32, // 0 stopped, 1 first recording, 2 playing, 3 overdubbing
    pub length: u32,
    pub position: u32,
    pub revision: u32,
    pub wraps: u32,
}
impl Looper {
    pub const fn new() -> Self {
        Self {
            state: 0,
            length: 0,
            position: 0,
            revision: 0,
            wraps: 0,
        }
    }
    pub fn capture(&self) -> bool {
        self.state == 1 || self.state == 3
    }
    fn finish(&mut self, play: bool) {
        if self.state == 1 {
            self.length = self.position;
            self.position = 0;
        }
        if self.capture() {
            self.revision = self.revision.wrapping_add(1);
        }
        self.state = if play && self.length > 0 { 2 } else { 0 };
    }
    pub fn stop(&mut self) {
        self.finish(false);
    }
    pub fn play(&mut self) {
        self.finish(true);
        self.position = 0;
    }
    pub fn record(&mut self) {
        match self.state {
            1 | 3 => self.finish(true),
            2 => self.state = 3,
            _ => {
                self.state = 1;
                self.length = 0;
                self.position = 0;
                self.wraps = 0;
            }
        }
    }
    pub fn clear(&mut self) {
        if self.state == 0 {
            self.length = 0;
            self.position = 0;
            self.wraps = 0;
            self.revision = self.revision.wrapping_add(1);
        }
    }
    pub fn restore(&mut self, length: u32) {
        if self.state == 0 && length <= CAPACITY {
            self.length = length;
            self.position = 0;
            self.wraps = 0;
            self.revision = self.revision.wrapping_add(1);
        }
    }
    pub unsafe fn sample(&mut self, input: u32) -> u32 {
        if self.state == 0 {
            return 0;
        }
        let input = (input as i32) << 8 >> 8;
        let address = (BUFFER + self.position * 4) as *mut i32;
        let output = match self.state {
            1 => {
                write_volatile(address, input);
                input
            }
            3 => {
                let value = (read_volatile(address) + input).clamp(-8_388_608, 8_388_607);
                write_volatile(address, value);
                value
            }
            _ => read_volatile(address),
        };
        self.position += 1;
        if self.state == 1 {
            if self.position == CAPACITY {
                self.finish(true);
            }
        } else if self.position >= self.length {
            self.position = 0;
            self.wraps = self.wraps.wrapping_add(1);
        }
        output as u32
    }
}
