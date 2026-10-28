//! A sheet over the bottom of a phone's window, the way a phone has one: a
//! handle across its top that it is dragged by — up to make it taller, down
//! to put it away — and the window dimmed behind it, which a press also puts
//! it away by.
//!
//! The toolkit's own sheet is a desktop panel, with a title bar and a small
//! close button to aim at; this one has neither. What it holds draws its own
//! title and its own commitment. Escape, and a phone's back, close it too:
//! it is a layer of [`gpui_android_glue::back`].
//!
//! The shell keeps them, a stack of them: a sheet opened from a sheet — a
//! new category from the record being written — opens over it, and closing
//! it uncovers the one under it as it was. The shell draws them over
//! everything it draws and under the toolkit's dialogs and popups.
//!
//! [`open_form`] is a form in a sheet: its title, its fields, Cancel and the
//! one commitment. On a phone every form opens so (`ui::open_form`).
//!
//! Where the sheet stands is one number, its height above the bottom of the
//! window: the height of what it holds when it rests, all the room there is
//! when it has been pulled up, less than what it holds — the rest of it below
//! the window — while it is dragged down or slides in or out. A drag sets the
//! height under the finger; letting go, opening and closing move it there
//! over a few frames, drawn one by one ([`Motion`]), so that what the sheet
//! holds keeps its own state the whole way.

use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::{ActiveTheme, FocusTrapElement, Sizable, StyledExt, h_flex, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::{
    AnyView, App, Bounds, Context, DispatchPhase, Entity, EntityId, FocusHandle, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, SharedString, TouchDragEvent,
    TouchPhase, Window, canvas, div, px,
};

/// How far down, in rems, the handle has to be dragged for the sheet to go
/// away when it is let go; short of that it springs back.
const PUT_AWAY_AT: f32 = 5.0;

/// The least of the window, in rems, left showing above the sheet.
const GAP_ABOVE: f32 = 2.0;

/// How long the sheet takes to come up from below the window.
const OPENING: Duration = Duration::from_millis(260);
/// How long it takes to go down out of the window.
const CLOSING: Duration = Duration::from_millis(200);
/// How long it takes to settle where a drag let it go.
const SETTLING: Duration = Duration::from_millis(180);

pub struct BottomSheet {
    content: AnyView,
    focus: FocusHandle,
    /// What had the focus before the sheet opened, which gets it back.
    previous: Option<FocusHandle>,
    /// The drag of the handle going on, if one is.
    drag: Option<Drag>,
    /// Whether it has been pulled up to all the room there is.
    expanded: bool,
    /// Whether it is on its way out.
    closing: bool,
    /// The way to where it is going, while it is on it.
    motion: Option<Motion>,
    /// How tall what it holds is, the handle and the room at the bottom
    /// included: its height at rest. Measured as it is drawn.
    natural: Rc<Cell<Pixels>>,
    /// All the room there is above the bottom of the window, as last drawn.
    full: Pixels,
    /// The room the window keeps at the bottom for the system's bar, which
    /// the sheet's content stays above. The shell says, as it draws.
    bar_room: Pixels,
    _back: gpui_android_glue::back::Guard,
}

/// A drag of the handle: where the finger or the pointer started, and how
/// tall the sheet was then.
#[derive(Clone, Copy)]
struct Drag {
    from: Pixels,
    height: Pixels,
    /// The height under the finger now.
    at: Pixels,
}

/// The sheet going somewhere over a few frames.
#[derive(Clone, Copy)]
struct Motion {
    /// The height it started from; `None` for out of sight below the window.
    from: Option<Pixels>,
    started: Instant,
    takes: Duration,
}

impl Motion {
    fn new(from: Option<Pixels>, takes: Duration) -> Self {
        Motion { from, started: Instant::now(), takes }
    }

    /// How far along it is, eased: fast at first, slow into its place.
    fn progress(&self) -> f32 {
        let t = (self.started.elapsed().as_secs_f32() / self.takes.as_secs_f32()).clamp(0., 1.);
        1. - (1. - t).powi(3)
    }

    fn done(&self) -> bool {
        self.started.elapsed() >= self.takes
    }
}

/// Opens `content` in a sheet, over any sheet that is open.
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
        drag: None,
        expanded: false,
        closing: false,
        motion: Some(Motion::new(None, OPENING)),
        natural: Rc::new(Cell::new(Pixels::ZERO)),
        full: Pixels::ZERO,
        bar_room: Pixels::ZERO,
        _back: gpui_android_glue::back::push(|window, cx| {
            let open = is_open(cx);
            close(window, cx);
            open
        }),
    });
    shell.update(cx, |shell, cx| shell.push_sheet(sheet, cx));
}

/// Closes the topmost sheet, if one is open: it slides down out of the
/// window, and the focus goes back to what had it before at once.
pub fn close(window: &mut Window, cx: &mut App) {
    let Some(sheet) = current(cx) else {
        return;
    };
    let previous = sheet.update(cx, |sheet, cx| {
        if sheet.closing {
            return None;
        }
        let from = sheet.height_now();
        sheet.closing = true;
        sheet.drag = None;
        sheet.motion = Some(Motion::new(Some(from), CLOSING));
        cx.notify();
        Some(sheet.previous.clone())
    });
    let Some(previous) = previous else {
        return;
    };
    if let Some(previous) = previous {
        previous.focus(window, cx);
    }
    let id = sheet.entity_id();
    cx.spawn(async move |cx| {
        cx.background_executor().timer(CLOSING).await;
        cx.update(|cx| take_away(id, cx));
    })
    .detach();
}

/// Takes the sheet `id` out of the shell.
fn take_away(id: EntityId, cx: &mut App) {
    if let Some(shell) = crate::shell::current(cx) {
        shell.update(cx, |shell, cx| shell.remove_sheet(id, cx));
    }
}

fn current(cx: &App) -> Option<Sheet> {
    crate::shell::current(cx).and_then(|shell| shell.read(cx).sheet().cloned())
}

/// Whether a sheet is open.
pub fn is_open(cx: &App) -> bool {
    crate::shell::current(cx).is_some_and(|shell| shell.read(cx).sheet().is_some())
}

/// Where the sheet stands at `height`, given how tall what it holds is: as
/// tall as that, taller (pulled up), or partly below the window. Answers the
/// least height to give it, if more than its own, and how far below the
/// bottom of the window it goes.
fn placement(height: Pixels, natural: Pixels) -> (Option<Pixels>, Pixels) {
    if height > natural {
        (Some(height), Pixels::ZERO)
    } else {
        (None, natural - height)
    }
}

impl BottomSheet {
    pub(crate) fn set_bar_room(&mut self, room: Pixels) {
        self.bar_room = room;
    }

    /// The height it rests at: all the room there is, or its own.
    fn resting(&self) -> Pixels {
        if self.expanded {
            self.full
        } else {
            self.natural.get()
        }
    }

    /// The height it is going to.
    fn target(&self) -> Pixels {
        if self.closing {
            // All of it below the window, and its shadow with it.
            self.natural.get().min(self.full) - self.full - px(48.)
        } else {
            self.resting()
        }
    }

    /// The height it stands at in this frame.
    fn height_now(&self) -> Pixels {
        if let Some(drag) = self.drag {
            return drag.at;
        }
        let target = self.target();
        let Some(motion) = self.motion else {
            return target;
        };
        let from = motion.from.unwrap_or(self.natural.get() - self.full - px(48.));
        from + (target - from) * motion.progress()
    }

    fn start_drag(&mut self, y: Pixels, cx: &mut Context<Self>) {
        if self.closing {
            return;
        }
        let height = self.height_now();
        self.motion = None;
        self.drag = Some(Drag { from: y, height, at: height });
        cx.notify();
    }

    fn drag_to(&mut self, y: Pixels, cx: &mut Context<Self>) {
        let full = self.full;
        if let Some(drag) = &mut self.drag {
            drag.at = (drag.height - (y - drag.from)).min(full);
            cx.notify();
        }
    }

    fn let_go(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(drag) = self.drag.take() else {
            return;
        };
        let natural = self.natural.get();
        if natural - drag.at > window.rem_size() * PUT_AWAY_AT {
            self.drag = Some(drag);
            // After this update: closing reads the sheet.
            window.defer(cx, close);
            return;
        }
        // Past halfway to the top it goes all the way up; short of that, back
        // to its own height.
        self.expanded = self.full > natural && drag.at > (natural + self.full) / 2.;
        self.motion = Some(Motion::new(Some(drag.at), SETTLING));
        cx.notify();
    }

    /// The handle: a short bar across the top, in a strip tall enough for a
    /// finger, which a drag starts on. A finger's drag is offered to it
    /// before it can become a scroll of what the sheet holds.
    fn render_handle(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let this = cx.entity().downgrade();
        div()
            .id("sheet-handle")
            .relative()
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
                    this.start_drag(event.position.y, cx);
                }),
            )
            .child(
                canvas(
                    |_, _, _| {},
                    move |bounds: Bounds<Pixels>, _, window, _| {
                        let this = this.clone();
                        window.on_mouse_event(move |event: &TouchDragEvent, phase, window, cx| {
                            if phase != DispatchPhase::Bubble {
                                return;
                            }
                            let taken = this
                                .update(cx, |sheet, cx| match event.phase {
                                    TouchPhase::Started => {
                                        if !bounds.contains(&event.start_position) {
                                            return false;
                                        }
                                        sheet.start_drag(event.position.y, cx);
                                        true
                                    }
                                    TouchPhase::Moved => {
                                        sheet.drag_to(event.position.y, cx);
                                        sheet.drag.is_some()
                                    }
                                    TouchPhase::Ended | TouchPhase::Cancelled => {
                                        let dragging = sheet.drag.is_some();
                                        sheet.let_go(window, cx);
                                        dragging
                                    }
                                })
                                .unwrap_or(false);
                            if taken {
                                window.prevent_default();
                                cx.stop_propagation();
                            }
                        });
                    },
                )
                .absolute()
                .size_full(),
            )
    }
}

impl Render for BottomSheet {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let rem = window.rem_size();
        let keyboard = crate::shell::keyboard_inset(window);
        let bottom = keyboard.max(self.bar_room);
        let room = window.viewport_size().height
            - keyboard
            - crate::shell::status_bar_inset()
            - rem * GAP_ABOVE;
        self.full = room.max(Pixels::ZERO) + bottom;

        if let Some(motion) = self.motion {
            if motion.done() {
                self.motion = None;
            } else {
                window.request_animation_frame();
            }
        }
        let natural = self.natural.get();
        let height = self.height_now();
        let (least, below) = placement(height, natural);
        let handle = self.render_handle(cx);
        let theme = cx.theme();
        // The window darkens as the sheet comes up, and lightens as it goes.
        let shown = if self.full > Pixels::ZERO {
            (1. - f32::from(below) / f32::from(self.full)).clamp(0., 1.)
        } else {
            1.
        };
        let mut overlay = theme.overlay;
        overlay.a *= shown;

        // How tall what the sheet holds is, measured as it is laid out: the
        // handle and the content, and the room at the bottom. Only while
        // nothing pulls it taller, which what it holds grows into.
        let measured = self.natural.clone();
        let pulled = least.is_some();
        let measure = canvas(
            move |bounds: Bounds<Pixels>, _, _| {
                if !pulled {
                    measured.set(bounds.size.height + bottom + px(1.));
                }
            },
            |_, _, _, _| {},
        )
        .absolute()
        .size_full();

        let surface = v_flex()
            .id("sheet-surface")
            .absolute()
            .left_0()
            .right_0()
            .bottom(-below)
            .when_some(least, |surface, least| surface.min_h(least))
            .max_h(self.full)
            .occlude()
            .bg(theme.background)
            .border_t_1()
            .border_color(theme.border)
            .rounded_t(theme.radius_lg * 2.)
            .shadow_xl()
            .pb(bottom)
            .child(
                v_flex()
                    .relative()
                    .flex_grow(1.)
                    .flex_shrink(1.)
                    .min_h_0()
                    .child(handle)
                    .child(
                        v_flex()
                            .flex_grow(1.)
                            .flex_shrink(1.)
                            .min_h_0()
                            .child(self.content.clone()),
                    )
                    .child(measure),
            )
            .focus_trap("sheet-focus", &self.focus);

        // While the handle is held by the pointer, it is followed wherever it
        // goes over the window, not only over the handle.
        let dragging = self.drag.is_some().then(|| {
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

/// Saves a form, answering whether it was acceptable.
type Submit = Rc<dyn Fn(&mut App) -> bool>;

/// A form in a sheet: its title at the top, its fields scrolling under it,
/// and Cancel and its one commitment across the foot, in view while the
/// fields scroll. Without a commitment, Done alone.
struct FormSheet {
    title: SharedString,
    form: AnyView,
    commit: Option<(SharedString, Submit)>,
}

/// Opens `form` in a sheet, over any sheet that is open: what `ui::open_form`
/// does on a phone. `submit` answers whether the form was acceptable; the
/// sheet goes away only if it was. `first` is focused once the sheet is up,
/// so that closing gives the focus back to what had it before.
pub fn open_form(
    title: SharedString,
    commit: SharedString,
    form: AnyView,
    first: Option<FocusHandle>,
    submit: Submit,
    window: &mut Window,
    cx: &mut App,
) {
    let sheet = cx.new(|_| FormSheet { title, form, commit: Some((commit, submit)) });
    open(sheet, window, cx);
    if let Some(first) = first {
        first.focus(window, cx);
    }
}

/// Opens `view` under `title` in a sheet with Done across its foot: a list
/// to look over and change in place, where a desktop has a dialog with Done.
pub fn open_panel(title: SharedString, view: AnyView, window: &mut Window, cx: &mut App) {
    let sheet = cx.new(|_| FormSheet { title, form: view, commit: None });
    open(sheet, window, cx);
}

impl Render for FormSheet {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let commit = self.commit.clone().map(|(label, submit)| {
            Button::new("ok")
                .label(label)
                .primary()
                .large()
                .flex_1()
                .on_click(move |_, window, cx| {
                    if submit(cx) {
                        close(window, cx);
                    }
                })
        });
        let dismiss = if commit.is_some() { "Cancel" } else { "Done" };
        v_flex()
            .flex_grow(1.)
            .min_h_0()
            .child(
                div()
                    .flex_none()
                    .px_4()
                    .pt_1()
                    .pb_3()
                    .text_lg()
                    .font_semibold()
                    .child(self.title.clone()),
            )
            .child(
                div()
                    .id("form-body")
                    .flex_grow(1.)
                    .flex_shrink(1.)
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(div().px_4().pb_4().child(self.form.clone())),
            )
            .child(
                h_flex()
                    .flex_none()
                    .gap_2()
                    .px_4()
                    .pt_3()
                    .pb_4()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .child(
                        Button::new("cancel")
                            .label(dismiss)
                            .outline()
                            .large()
                            .flex_1()
                            .on_click(|_, window, cx| close(window, cx)),
                    )
                    .children(commit),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sheet_taller_than_its_own_is_given_the_height_and_one_shorter_goes_below() {
        assert_eq!(placement(px(300.), px(300.)), (None, px(0.)));
        assert_eq!(placement(px(500.), px(300.)), (Some(px(500.)), px(0.)));
        assert_eq!(placement(px(200.), px(300.)), (None, px(100.)));
    }
}
