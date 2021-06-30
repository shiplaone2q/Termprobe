use super::*;
use crate::profile::Profile;
use crate::render::svg::RenderState;
use crate::terminal::alacritty::AlacrittyEmu;
use crate::terminal::emu::Emulator;

pub(super) const HOSTILE: &str = "</script><script>globalThis.reportPwned=true</script>\n| ``` ` & <img src=x onerror=alert(1)> /* REPORT_JS */ null /* REPORT_DATA */";

fn capture(history: &mut ScreenHistory, emu: &AlacrittyEmu, time: u64) -> u64 {
    history.capture(
        emu.viewable_rows(),
        emu.size().0,
        emu.title(),
        emu.cursor(),
        emu.cursor_visible(),
        Emulator::cursor_shape(emu),
        time,
        RenderState::capture(emu),
    )
}

pub(super) fn fixture() -> (FailureReport, FailureObservation) {
    let mut emu = AlacrittyEmu::new(20, 4, &Profile::default());
    let mut history = ScreenHistory::new(10);
    emu.process(b"\x1b[?25lREADY");
    capture(&mut history, &emu, 10);
    history.pin_current();
    capture(&mut history, &emu, 12);
    emu.process(b"\x1b[2J\x1b[HWORKING");
    capture(&mut history, &emu, 20);
    emu.resize(24, 5);
    emu.process("\x1b]4;1;#112233\x07\x1b[2J\x1b[H\x1b[31mA\x1b[0m\u{4f60}e\u{301}".as_bytes());
    emu.process(b"\x1b]2;</script><img src=x onerror=alert(1)>\x07");
    let sequence = capture(&mut history, &emu, 30);
    capture(&mut history, &emu, 40);
