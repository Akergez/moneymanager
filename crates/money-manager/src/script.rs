//! A scripted pair of hands, for looking at the interface without a human.
//!
//! `MONEY_MANAGER_SCRIPT="wait:1500;key:ctrl-2;expect:screen=categories;quit"`
//! plays those steps into the window once it opens. Nothing here is reachable
//! without the variable; it exists because a Wayland session gives a test no
//! other way to press a button and see what the screen looks like afterwards.
//!
//! Steps:
//! - `wait:<ms>`
//! - `click:<x>,<y>`, `rclick:<x>,<y>`, `move:<x>,<y>`
//! - `scroll:<x>,<y>,<dy>`
//! - `key:<keystroke>` — `ctrl-n`, `escape`, `enter`
//! - `type:<text>`
//! - `shot:<name>` — runs `$MONEY_MANAGER_SHOT_CMD <name>`
//! - `expect:<what>=<value>` — ends the process with status 1 unless it holds
//! - `quit`
//!
//! `expect` is what turns a script into a test (`tests/ui/` and
//! `build-aux/ui-tests.sh`): a click that lands beside its button changes
//! nothing, and without a check the script would still run to its `quit`. It
//! reads the application's state, not the picture, so it does not depend on
//! fonts or on how a compositor draws:
//! - `stage=starting|welcome|workspace|failed`
//! - `onboarding=choose|create|connect` — which step the first-run form is on
//! - `screen=transactions|categories|charts`
//! - `mode=expense|income`
//! - `layout=phone|tablet|desktop`
//! - `account=<name>` — the account every screen is about
//! - `accounts=<n>`, `records=<n>`, `categories=<n>` — how many there are, the
//!   last two of the current kind
//! - `balance=<amount>` — the current account's, as the top bar writes it
//! - `bars-hidden=<n>`, `line-hidden=<n>` — how many categories the monthly
//!   bars and the running total each leave out
//! - `dialog=open|closed`
//! - `theme=light|dark`
//! - `sync=idle|running|done|failed`
//! - `remote=yes|no` — whether a sync storage is set up
//!
//! Coordinates are divided by `MONEY_MANAGER_SCRIPT_SCALE`, so they can be
//! read straight off a screenshot taken on a scaled display.

use std::collections::VecDeque;
use std::time::Duration;

use gpui_kit::component::{ActiveTheme as _, WindowExt as _};
use gpui_kit::{
    AnyWindowHandle, App, Keystroke, Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, Pixels, PlatformInput, Point, ScrollDelta, ScrollWheelEvent, TouchPhase, Window,
    point, px,
};
use money_core::format::format_money;

use crate::book::{Mode, SyncState};
use crate::settings::Settings;
use crate::shell::Stage;

#[derive(Debug, Clone, PartialEq)]
enum Step {
    Wait(u64),
    Click(f32, f32, bool),
    Move(f32, f32),
    Scroll(f32, f32, f32),
    Key(String),
    Type(String),
    Shot(String),
    Expect(String, String),
    Quit,
}

fn numbers(text: &str) -> Option<Vec<f32>> {
    text.split(',').map(|n| n.trim().parse().ok()).collect()
}

fn parse(script: &str) -> Vec<Step> {
    script
        .split(';')
        .filter_map(|step| {
            let step = step.trim();
            let (verb, rest) = step.split_once(':').unwrap_or((step, ""));
            Some(match verb {
                "wait" => Step::Wait(rest.parse().ok()?),
                "click" | "rclick" => match numbers(rest)?[..] {
                    [x, y] => Step::Click(x, y, verb == "rclick"),
                    _ => return None,
                },
                "move" => match numbers(rest)?[..] {
                    [x, y] => Step::Move(x, y),
                    _ => return None,
                },
                "scroll" => match numbers(rest)?[..] {
                    [x, y, dy] => Step::Scroll(x, y, dy),
                    _ => return None,
                },
                "key" => Step::Key(rest.to_string()),
                "type" => Step::Type(rest.to_string()),
                "shot" => Step::Shot(rest.to_string()),
                "expect" => {
                    let (what, value) = rest.split_once('=')?;
                    Step::Expect(what.trim().to_string(), value.trim().to_string())
                }
                "quit" => Step::Quit,
                _ => {
                    tracing::warn!(step, "unknown script step");
                    return None;
                }
            })
        })
        .collect()
}

/// Plays `MONEY_MANAGER_SCRIPT` into `window`, if there is one to play.
pub fn play(window: AnyWindowHandle, cx: &mut App) {
    let Ok(script) = std::env::var("MONEY_MANAGER_SCRIPT") else {
        return;
    };
    next(window, parse(&script).into(), cx);
}

/// Coordinates are read off screenshots, which are in device pixels;
/// `MONEY_MANAGER_SCRIPT_SCALE` is the factor between those and the window's
/// own.
fn at(x: f32, y: f32) -> Point<Pixels> {
    let scale = std::env::var("MONEY_MANAGER_SCRIPT_SCALE")
        .ok()
        .and_then(|scale| scale.parse::<f32>().ok())
        .filter(|scale| *scale > 0.0)
        .unwrap_or(1.0);
    point(px(x / scale), px(y / scale))
}

fn pointer(window: &mut Window, position: Point<Pixels>, cx: &mut App) {
    window.dispatch_event(
        PlatformInput::MouseMove(MouseMoveEvent {
            position,
            pressed_button: None,
            modifiers: Modifiers::default(),
        }),
        cx,
    );
}

/// What `expect:<what>` compares against, or `None` for a name it does not
/// know — which fails the script too, so a misspelt check cannot pass.
fn observe(what: &str, window: &mut Window, cx: &mut App) -> Option<String> {
    let shell = crate::shell::current(cx)?;
    let (stage, onboarding, workspace) = match shell.read(cx).stage() {
        Stage::Starting => ("starting", None, None),
        Stage::Welcome(onboarding) => ("welcome", Some(onboarding.clone()), None),
        Stage::Workspace(workspace) => ("workspace", None, Some(workspace.clone())),
        Stage::Failed(_) => ("failed", None, None),
    };
    // The ones that are about the window rather than about a ledger.
    match what {
        "stage" => return Some(stage.to_string()),
        "dialog" => {
            let open = window.has_active_dialog(cx);
            return Some(if open { "open" } else { "closed" }.to_string());
        }
        "theme" => {
            let dark = cx.theme().mode.is_dark();
            return Some(if dark { "dark" } else { "light" }.to_string());
        }
        "remote" => {
            let set = Settings::global(cx).remote().is_some();
            return Some(if set { "yes" } else { "no" }.to_string());
        }
        "onboarding" => {
            return Some(
                match onboarding?.read(cx).step() {
                    crate::onboarding::Step::Choose => "choose",
                    crate::onboarding::Step::Create => "create",
                    crate::onboarding::Step::Connect => "connect",
                }
                .to_string(),
            );
        }
        _ => {}
    }

    let workspace = workspace?;
    let workspace = workspace.read(cx);
    let book = workspace.book().read(cx);
    let mode = workspace.mode();
    Some(match what {
        "screen" => workspace.screen().name().to_string(),
        "mode" => match mode {
            Mode::Expense => "expense",
            Mode::Income => "income",
        }
        .to_string(),
        "layout" => workspace.layout().name().to_string(),
        "account" => book
            .current_account()
            .map(|account| account.name.clone())
            .unwrap_or_default(),
        "accounts" => book.accounts().len().to_string(),
        "records" => book.entries(mode).len().to_string(),
        "categories" => book.categories(mode).len().to_string(),
        "bars-hidden" => workspace.charts().read(cx).hidden_counts().0.to_string(),
        "line-hidden" => workspace.charts().read(cx).hidden_counts().1.to_string(),
        "balance" => book
            .current_account()
            .map(|account| format_money(book.summary(&account.id).balance, &account.currency))
            .unwrap_or_default(),
        "sync" => match book.sync_state() {
            SyncState::Idle => "idle",
            SyncState::Running => "running",
            SyncState::Done { .. } => "done",
            SyncState::Failed(_) => "failed",
        }
        .to_string(),
        _ => return None,
    })
}

fn next(handle: AnyWindowHandle, mut steps: VecDeque<Step>, cx: &mut App) {
    let Some(step) = steps.pop_front() else {
        return;
    };
    // Every step is followed by a short pause: what a step does is usually
    // queued rather than done, and the next one should see its result.
    let mut pause = 120;
    tracing::info!(?step, "script");
    let _ = handle.update(cx, |_, window, cx| match &step {
        Step::Wait(ms) => pause = *ms,
        Step::Move(x, y) => pointer(window, at(*x, *y), cx),
        Step::Click(x, y, right) => {
            let position = at(*x, *y);
            let button = if *right {
                MouseButton::Right
            } else {
                MouseButton::Left
            };
            pointer(window, position, cx);
            window.dispatch_event(
                PlatformInput::MouseDown(MouseDownEvent {
                    button,
                    position,
                    modifiers: Modifiers::default(),
                    click_count: 1,
                    first_mouse: false,
                }),
                cx,
            );
            window.dispatch_event(
                PlatformInput::MouseUp(MouseUpEvent {
                    button,
                    position,
                    modifiers: Modifiers::default(),
                    click_count: 1,
                }),
                cx,
            );
        }
        Step::Scroll(x, y, dy) => {
            pointer(window, at(*x, *y), cx);
            window.dispatch_event(
                PlatformInput::ScrollWheel(ScrollWheelEvent {
                    position: at(*x, *y),
                    delta: ScrollDelta::Pixels(point(px(0.), px(*dy))),
                    modifiers: Modifiers::default(),
                    touch_phase: TouchPhase::Moved,
                }),
                cx,
            );
        }
        Step::Key(key) => match Keystroke::parse(key) {
            Ok(keystroke) => {
                window.dispatch_keystroke(keystroke, cx);
            }
            Err(error) => tracing::warn!(%error, key, "not a keystroke"),
        },
        Step::Type(text) => {
            for letter in text.chars() {
                let keystroke = Keystroke {
                    modifiers: Modifiers::default(),
                    key: letter.to_string(),
                    key_char: Some(letter.to_string()),
                };
                window.dispatch_keystroke(keystroke, cx);
            }
        }
        Step::Shot(name) => {
            if let Ok(command) = std::env::var("MONEY_MANAGER_SHOT_CMD") {
                let _ = std::process::Command::new(command).arg(name).status();
            }
        }
        Step::Expect(what, value) => {
            let found = observe(what, window, cx);
            if found.as_deref() != Some(value) {
                // Straight out, with a status the runner can see: the steps
                // after a failed check would only be acting on the wrong screen.
                eprintln!("script: expected {what}={value}, found {found:?}");
                std::process::exit(1);
            }
        }
        Step::Quit => cx.quit(),
    });
    cx.spawn(async move |cx| {
        cx.background_executor()
            .timer(Duration::from_millis(pause))
            .await;
        cx.update(|cx| next(handle, steps, cx));
    })
    .detach();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_script_is_steps_between_semicolons() {
        assert_eq!(
            parse(
                "wait:200; click:10,20 ;rclick:1,2;key:ctrl-n;type:a:b;shot:x;expect:account=Main card;quit"
            ),
            [
                Step::Wait(200),
                Step::Click(10.0, 20.0, false),
                Step::Click(1.0, 2.0, true),
                Step::Key("ctrl-n".into()),
                Step::Type("a:b".into()),
                Step::Shot("x".into()),
                Step::Expect("account".into(), "Main card".into()),
                Step::Quit,
            ]
        );
    }

    #[test]
    fn a_step_that_does_not_parse_is_skipped() {
        assert_eq!(
            parse("click:1;bogus;wait:x;scroll:1,2,3"),
            [Step::Scroll(1.0, 2.0, 3.0)]
        );
    }
}
