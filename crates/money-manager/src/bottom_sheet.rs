//! A sheet over the bottom of a phone's window, the way a phone has one: a
//! handle across its top that it is dragged down by to put it away, and the
//! window dimmed behind it, which a press also puts it away by.
//!
//! The toolkit's own sheet is a desktop panel, with a title bar and a small
//! close button to aim at; this one has neither. What it holds draws its own
//! title and its own commitment. Escape, and a phone's back, close it too:
//! it is a layer of [`gpui_android_glue::back`].
//!
//! The window keeps one at a time, in the shell, which draws it over
//! everything it draws and under the toolkit's dialogs and popups.

use std::time::Duration;

use gpui_kit::component::{ActiveTheme, FocusTrapElement, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::{
    Animation, AnimationExt, AnyView, App, Context, DispatchPhase, Entity, FocusHandle,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Window, canvas, div, px,
};

/// How far down, in rems, the handle has to be dragged for the sheet to go
/// away when it is let go; short of that it springs back.
const PUT_AWAY_AT: f32 = 5.0;

/// The least of the window, in rems, left showing above the sheet.
const GAP_ABOVE: f32 = 2.0;

pub struct BottomSheet {
    content: AnyView,
    focus: FocusHandle,
    /// What had the focus before the sheet opened, which gets it back.
    previous: Option<FocusHandle>,
    /// Where a drag of the handle started, while one is going on.
    drag_from: Option<Pixels>,
    /// How far the sheet has been dragged down.
    offset: Pixels,
    /// The room the window keeps at the bottom for the system's bar, which
    /// the sheet's content stays above. The shell says, as it draws.
    bar_room: Pixels,
    _back: gpui_android_glue::back::Guard,
}

/// Opens `content` in a sheet, in place of any sheet that is open.
pub fn open(content: impl Into<AnyView>, window: &mut Window, cx: &mut App) {
    let Some(shell) = crate::shell::current(cx) else {
        return;
    };
    let previous = window.focused(cx);
    let content = content.into();
    let sheet = cx.new(|cx| BottomSheet {
        content,
        focus: cx.focus_handle(),
        previous,
        drag_from: None,
        offset: Pixels::ZERO,
        bar_room: Pixels::ZERO,
        _back: gpui_android_glue::back::push(|window, cx| {
            let open = is_open(cx);
            close(window, cx);
            open
        }),
    });
    shell.update(cx, |shell, cx| shell.set_sheet(Some(sheet), cx));
}

/// Closes the sheet, if one is open, and gives the focus back to what had it
/// before.
pub fn close(window: &mut Window, cx: &mut App) {
    let Some(shell) = crate::shell::current(cx) else {
        return;
    };
    let Some(sheet) = shell.update(cx, |shell, cx| shell.set_sheet(None, cx)) else {
        return;
    };
    if let Some(previous) = sheet.read(cx).previous.clone() {
        previous.focus(window, cx);
    }
}

/// Whether a sheet is open.
pub fn is_open(cx: &App) -> bool {
    crate::shell::current(cx).is_some_and(|shell| shell.read(cx).sheet().is_some())
}

impl BottomSheet {
    pub(crate) fn set_bar_room(&mut self, room: Pixels) {
        self.bar_room = room;
    }

    fn drag_to(&mut self, y: Pixels, cx: &mut Context<Self>) {
        if let Some(from) = self.drag_from {
            self.offset = (y - from).max(Pixels::ZERO);
            cx.notify();
        }
    }

    fn let_go(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.drag_from.take().is_none() {
            return;
        }
        if self.offset > window.rem_size() * PUT_AWAY_AT {
            // After this update: closing takes the sheet out of the shell.
            window.defer(cx, close);
        } else {
            self.offset = Pixels::ZERO;
            cx.notify();
        }
    }

    /// The handle: a short bar across the top, in a strip tall enough for a
    /// finger, which a drag starts on.
    fn render_handle(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        div()
            .id("sheet-handle")
            .flex_none()
            .w_full()
            .h_6()
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .w_10()
                    .h_1()
                    .rounded_full()
                    .bg(cx.theme().muted_foreground.opacity(0.4)),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, _, cx| {
                    this.drag_from = Some(event.position.y - this.offset);
                    cx.notify();
                }),
            )
    }
}

impl Render for BottomSheet {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let rem = window.rem_size();
        let keyboard = crate::shell::keyboard_inset(window);
        let room = window.viewport_size().height
            - keyboard
            - crate::shell::status_bar_inset()
            - rem * GAP_ABOVE;
        let offset = self.offset;
        let handle = self.render_handle(cx);
        let theme = cx.theme();
        let overlay = theme.overlay;

        let surface = v_flex()
            .id("sheet-surface")
            .absolute()
            .left_0()
            .right_0()
            .bottom(-offset)
            .max_h(room.max(Pixels::ZERO) + keyboard.max(self.bar_room))
            .occlude()
            .bg(theme.background)
            .border_t_1()
            .border_color(theme.border)
            .rounded_t(theme.radius_lg * 2.)
            .shadow_xl()
            .pb(keyboard.max(self.bar_room))
            .child(handle)
            .child(
                v_flex()
                    .flex_shrink(1.)
                    .min_h_0()
                    .child(self.content.clone()),
            )
            .focus_trap("sheet-focus", &self.focus)
            .with_animation(
                "sheet-slide",
                Animation::new(Duration::from_millis(180)),
                move |surface, delta| surface.bottom(px(-120.) * (1. - delta) - offset),
            );

        // While the handle is held, the pointer is followed wherever it goes
        // over the window, not only over the handle.
        let dragging = self.drag_from.is_some().then(|| {
            let this = cx.entity().downgrade();
            canvas(
                |_, _, _| {},
                move |_, _, window, _| {
                    let moved = this.clone();
                    window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
                        if phase == DispatchPhase::Bubble {
                            moved
                                .update(cx, |this, cx| this.drag_to(event.position.y, cx))
                                .ok();
                        }
                    });
                    let released = this.clone();
                    window.on_mouse_event(move |_: &MouseUpEvent, phase, window, cx| {
                        if phase == DispatchPhase::Bubble {
                            released.update(cx, |this, cx| this.let_go(window, cx)).ok();
                        }
                    });
                },
            )
            .absolute()
            .size_full()
        });

        div()
            .id("sheet-scrim")
            .absolute()
            .inset_0()
            .occlude()
            .bg(overlay)
            // A press on the dimmed window, not on the sheet, puts it away.
            .on_mouse_down(MouseButton::Left, |_, window, cx| close(window, cx))
            .child(surface)
            .children(dragging)
    }
}

/// The entity behind a sheet, for the shell to hold.
pub type Sheet = Entity<BottomSheet>;
