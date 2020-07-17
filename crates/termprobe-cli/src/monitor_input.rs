use vte::{Params, Parser, Perform};

const MAX_PENDING: usize = 4096;
const CTRL_RIGHT_BRACKET: u8 = 0x1d;

pub(crate) enum InputEvent {
    Detach,
    Mouse {
        button: u16,
        x: u16,
        y: u16,
        release: bool,
    },
}

pub(crate) enum InputAction {
    Forward,
    Replace(Vec<u8>),
    Detach,
}

#[derive(Default)]
struct InputState {
    complete: bool,
    event: Option<InputEvent>,
    paste: bool,
}

impl Perform for InputState {
    fn print(&mut self, _: char) {
        self.complete = true;
    }

    fn execute(&mut self, byte: u8) {
        self.complete = true;
        if byte == CTRL_RIGHT_BRACKET && !self.paste {
            self.event = Some(InputEvent::Detach);
        }
    }

    fn csi_dispatch(&mut self, params: &Params, intermediates: &[u8], ignore: bool, action: char) {
        self.complete = true;
        if ignore {
            return;
        }
        let mut params = params.iter();
        let key = params.next().unwrap_or(&[]);
        if intermediates.is_empty() && action == '~' && params.next().is_none() {
            match key {
                [200] => self.paste = true,
                [201] => self.paste = false,
                _ => {}
            }
            return;
        }
        if self.paste {
            return;
        }
