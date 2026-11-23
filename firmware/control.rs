//! Shared with the no_std ARM firmware. Values describe the proposed board.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Play,
    Record,
    Stop,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Stopped,
    Starting { record: bool, since: u32 },
    Playing,
    Recording,
    Fault,
}
pub struct Controller {
    pub state: State,
}
impl Controller {
    pub const fn new() -> Self {
        Self {
            state: State::Stopped,
        }
    }
    pub fn tick(&mut self, now_us: u32, request: Option<Command>, media: bool, ready: bool) -> u16 {
        if let Some(command) = request {
            match command {
                Command::Stop => self.state = State::Stopped,
                Command::Play | Command::Record if media && self.state != State::Fault => {
                    self.state = State::Starting {
                        record: command == Command::Record,
                        since: now_us,
                    };
                }
                _ => self.state = State::Fault,
            }
        }
        if self.state != State::Stopped && !media {
            self.state = State::Fault;
        }
        if let State::Starting { record, since } = self.state {
            if now_us.wrapping_sub(since) > 1_000_000 {
                self.state = State::Fault;
            } else if ready {
                self.state = if record {
                    State::Recording
                } else {
                    State::Playing
                };
            }
        } else if matches!(self.state, State::Playing | State::Recording) && !ready {
            self.state = State::Fault;
        }
        match self.state {
            State::Starting { .. } | State::Playing => 0b0011, // motor, head engage
            State::Recording => 0b1111, // motor, engage, record gate, erase gate
            State::Stopped | State::Fault => 0,
        }
    }
}
impl Default for Controller {
    fn default() -> Self {
        Self::new()
    }
}
