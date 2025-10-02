use anyhow::{bail, ensure, Result};
use log::debug;
use rusb::{Context, DeviceHandle};

use crate::keyboard::Accord;

use super::{Key, Keyboard, Macro, MouseAction, MouseEvent};

pub struct Keyboard884x {
    handle: DeviceHandle<Context>,
    endpoint: u8,
}

impl Keyboard for Keyboard884x {
    fn bind_key(&mut self, layer: u8, key: Key, expansion: &Macro) -> Result<()> {
        ensure!(layer <= 15, "invalid layer index");

        debug!("bind {} on layer {} to {}", key, layer, expansion);

        let mut msg = vec![
            0x03,
            0xfe,
            key.to_key_id(15)?,
            layer + 1,
            expansion.kind(),
            0,
            0,
            0,
            0,
            0,
        ];

        match expansion {
            Macro::Keyboard(presses) => {
                ensure!(presses.len() <= 18, "macro sequence is too long");

                // Count only key parts when putting header length
                let key_count = presses.iter().filter(|p| matches!(p, super::KeyboardPart::Key(_))).count();

                // Use actual key count. Using 0 for single-key breaks cases with a leading delay.
                msg.push(key_count as u8);

                for part in presses.iter() {
                    match part {
                        super::KeyboardPart::Key(Accord { modifiers, code }) => {
                            msg.extend_from_slice(&[modifiers.as_u8(), code.map_or(0, |c| c.value())]);
                        }
                        super::KeyboardPart::Delay(_) => {
                            // Delay entries are not part of the header payload for key programming.
                        }
                    }
                }
            }
            Macro::Media(code) => {
                let [low, high] = (*code as u16).to_le_bytes();
                msg.extend_from_slice(&[0, low, high, 0, 0, 0, 0]);
            }
            Macro::Mouse(MouseEvent(MouseAction::Click(buttons), _)) => {
                ensure!(!buttons.is_empty(), "buttons must be given for click macro");
                msg.extend_from_slice(&[0x01, 0, buttons.as_u8()]);
            }
            Macro::Mouse(MouseEvent(MouseAction::WheelUp, modifier)) => {
                msg.extend_from_slice(&[0x03, modifier.map_or(0, |m| m as u8), 0, 0, 0, 0x1]);
            }
            Macro::Mouse(MouseEvent(MouseAction::WheelDown, modifier)) => {
                msg.extend_from_slice(&[0x03, modifier.map_or(0, |m| m as u8), 0, 0, 0, 0xff]);
            }
        };

        // Send main programming message (keys/media/mouse)
        self.send(&msg)?;

        // If macro has a leading delay part (we validated earlier that any delay must be leading),
        // send a single delay message with the specified ms after programming the macro.
        if let Macro::Keyboard(parts) = expansion {
            if let Some(super::KeyboardPart::Delay(ms)) = parts.first() {
                if *ms > 6000 {
                    return Err(anyhow::anyhow!("delay value {ms}ms exceeds maximum supported 6000ms"));
                }
                let mut delay_msg = msg.clone();
                delay_msg[4] = 0x05;
                let [low, high] = ms.to_le_bytes();
                delay_msg[5] = low;
                delay_msg[6] = high;
                self.send(&delay_msg)?;
            }
        }

        // Finish key binding
        self.send(&[0x03, 0xaa, 0xaa, 0, 0, 0, 0, 0, 0])?;
        self.send(&[0x03, 0xfd, 0xfe, 0xff])?;
        self.send(&[0x03, 0xaa, 0xaa, 0, 0, 0, 0, 0, 0])?;

        Ok(())
    }

    fn set_led(&mut self, _n: u8) -> Result<()> {
        bail!(
            "If you have a device which supports backlight LEDs, please let us know at \
               https://github.com/kriomant/ch57x-keyboard-tool/issues/60. We'll be glad to \
               help you reverse-engineer it."
        )
    }

    fn get_handle(&self) -> &DeviceHandle<Context> {
        &self.handle
    }

    fn get_endpoint(&self) -> u8 {
        self.endpoint
    }

    fn preferred_endpoint() -> u8 {
        0x04
    }
}

impl Keyboard884x {
    pub fn new(handle: DeviceHandle<Context>, endpoint: u8) -> Result<Self> {
        let mut keyboard = Self { handle, endpoint };

        keyboard.send(&[])?;

        Ok(keyboard)
    }
}
