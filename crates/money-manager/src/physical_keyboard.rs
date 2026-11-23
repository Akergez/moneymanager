//! Android delivers hardware keys on the Activity thread. Send them to the
//! GPUI window so its focused input and key bindings see the same event.

use std::ffi::c_void;
use std::sync::OnceLock;

use async_channel::Sender;
use gpui_kit::{App, KeyDownEvent, KeyUpEvent, Keystroke, PlatformInput, Window};
use gpui_mobile::android::keyboard::android_key_to_keystroke;

struct PhysicalKey {
    keystroke: Keystroke,
    down: bool,
    repeat: bool,
}

impl PhysicalKey {
    fn dispatch(self, window: &mut Window, cx: &mut App) {
        if self.down && self.keystroke.key_char.is_some() {
            window.dispatch_keystroke(self.keystroke, cx);
        } else {
            let event = if self.down {
                PlatformInput::KeyDown(KeyDownEvent {
                    keystroke: self.keystroke,
                    is_held: self.repeat,
                    prefer_character_input: false,
                })
            } else {
                PlatformInput::KeyUp(KeyUpEvent {
                    keystroke: self.keystroke,
                })
            };
            window.dispatch_event(event, cx);
        }
    }
}

static KEYS: OnceLock<Sender<PhysicalKey>> = OnceLock::new();

pub(crate) fn init(cx: &mut App) {
    let (sender, receiver) = async_channel::unbounded::<PhysicalKey>();
    if KEYS.set(sender).is_err() {
        return;
    }
    cx.spawn(async move |cx| {
        while let Ok(key) = receiver.recv().await {
            cx.update(|cx| {
                if let Some(window) = cx.active_window().or_else(|| cx.windows().first().copied()) {
                    let _ = window.update(cx, |_, window, cx| key.dispatch(window, cx));
                }
            });
        }
    })
    .detach();
}

// JNI receives jboolean as an unsigned byte. A zero result leaves the key
// with the Activity, so its existing system handling still runs.
#[unsafe(no_mangle)]
extern "system" fn Java_app_akergez_moneymanager_MoneyManagerActivity_nativePhysicalKey(
    _env: *mut c_void,
    _class: *mut c_void,
    code: i32,
    modifiers: i32,
    character: i32,
    down: u8,
    repeat: u8,
) -> u8 {
    let Some(sender) = KEYS.get() else {
        return 0;
    };
    // A negative value carries Android's combining-accent flag. Leave it
    // to the Android input path.
    if character < 0 {
        return 0;
    }
    let character = character as u32;
    let Some(mut keystroke) = android_key_to_keystroke(code, modifiers, character) else {
        return 0;
    };
    // The platform mapper also guesses characters from US key names. Only
    // insert text here if the real device produced a printable character.
    keystroke.key_char = char::from_u32(character)
        .filter(|character| !character.is_control())
        .map(|character| character.to_string());
    u8::from(
        sender
            .try_send(PhysicalKey {
                keystroke,
                down: down != 0,
                repeat: repeat != 0,
            })
            .is_ok(),
    )
}
