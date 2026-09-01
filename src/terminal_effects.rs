use std::io::{self, Write};

const BELL_CHUNK: [u8; 64] = [b'\x07'; 64];

pub(crate) fn write_terminal_bells<W: Write>(writer: &mut W, count: u16) -> io::Result<()> {
    let full_chunks = usize::from(count) / BELL_CHUNK.len();
    let remainder = usize::from(count) % BELL_CHUNK.len();
    for _ in 0..full_chunks {
        writer.write_all(&BELL_CHUNK)?;
    }
    writer.write_all(&BELL_CHUNK[..remainder])?;
    writer.flush()
}

pub(crate) fn write_window_title<W: Write>(writer: &mut W, title: Option<&str>) -> io::Result<()> {
    let title = title.unwrap_or("herdr");
    let safe_title = title
        .chars()
        .filter(|ch| !matches!(*ch, '\u{1b}' | '\u{7}' | '\u{9c}'))
        .collect::<String>();
    write!(writer, "\x1b]0;{safe_title}\x07")?;
    writer.flush()
}

/// Fork: set the host terminal's mouse pointer shape (OSC 22, as Ghostty and
/// kitty read it). An empty shape restores the terminal's own default.
pub(crate) fn write_mouse_shape<W: Write>(writer: &mut W, pointer: bool) -> io::Result<()> {
    let shape = if pointer { "pointer" } else { "default" };
    write!(writer, "\x1b]22;{shape}\x1b\\")?;
    writer.flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mouse_shape_switches_between_the_hand_and_the_default() {
        let mut output = Vec::new();
        write_mouse_shape(&mut output, true).unwrap();
        write_mouse_shape(&mut output, false).unwrap();

        assert_eq!(
            String::from_utf8(output).unwrap(),
            "\x1b]22;pointer\x1b\\\x1b]22;default\x1b\\"
        );
    }

    #[test]
    fn writes_exact_terminal_bell_count() {
        let mut output = Vec::new();

        write_terminal_bells(&mut output, 130).unwrap();

        assert_eq!(output, vec![b'\x07'; 130]);
    }

    #[test]
    fn window_title_strips_terminators_and_defaults_to_herdr() {
        let mut output = Vec::new();
        write_window_title(&mut output, Some("herdr\x1b api\u{7}\u{9c}")).unwrap();
        assert_eq!(output, b"\x1b]0;herdr api\x07");

        output.clear();
        write_window_title(&mut output, None).unwrap();
        assert_eq!(output, b"\x1b]0;herdr\x07");
    }
}
