//! Small pieces every screen draws with.
//!
//! Nothing here holds state or knows what the ledger is. These exist so that
//! the same thing looks the same in the list, the category grid and the
//! charts without three copies of how.

use std::rc::Rc;
use std::sync::{Arc, OnceLock};

use gpui_kit::base::actions::Confirm;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::dialog::{DialogAction, DialogClose, DialogFooter};
use gpui_kit::component::{ActiveTheme, Icon, Sizable, StyledExt, WindowExt, h_flex, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::{
    AnyElement, App, Context, Div, Entity, FocusHandle, Image, ImageFormat, SharedString, Window,
    div,
};

use crate::book::CategoryLook;

/// The full icon catalogue. The component library's own enum only carries the
/// hundred icons its own widgets use.
pub use gpui_kit::assets::IconName as Lucide;

/// The application's own icon — the same file the desktop shows in its
/// launcher.
pub fn app_icon() -> Arc<Image> {
    static ICON: OnceLock<Arc<Image>> = OnceLock::new();
    ICON.get_or_init(|| {
        Arc::new(Image::from_bytes(
            ImageFormat::Svg,
            include_bytes!(
                "../../../data/icons/hicolor/scalable/apps/app.akergez.MoneyManager.svg"
            )
            .to_vec(),
        ))
    })
    .clone()
}

/// One option of a select that stands for a record: an account or a category,
/// chosen by name and identified by id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Choice {
    pub id: Vec<u8>,
    pub title: SharedString,
}

impl Choice {
    pub fn new(id: Vec<u8>, title: impl Into<SharedString>) -> Self {
        Choice {
            id,
            title: title.into(),
        }
    }
}

impl gpui_kit::component::select::SelectItem for Choice {
    type Value = Vec<u8>;

    fn title(&self) -> SharedString {
        self.title.clone()
    }

    fn value(&self) -> &Self::Value {
        &self.id
    }
}

/// How large a category's tile is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TileSize {
    /// Beside a name in a table row.
    Row,
    /// Leading a list row or a card.
    Card,
    /// The preview at the top of the category editor.
    Preview,
}

/// A category's icon on a tint of its colour: what identifies it wherever it
/// appears.
pub fn category_tile(look: CategoryLook, size: TileSize, cx: &App) -> Div {
    let tile = div()
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .bg(look.color.opacity(0.16))
        .text_color(look.color);
    match size {
        TileSize::Row => tile
            .size_6()
            .rounded(cx.theme().radius / 2.)
            .child(Icon::new(look.icon).xsmall()),
        TileSize::Card => tile
            .size_10()
            .rounded(cx.theme().radius)
            .child(Icon::new(look.icon)),
        TileSize::Preview => tile
            .size_12()
            .rounded(cx.theme().radius_lg)
            .child(Icon::new(look.icon).large()),
    }
}

/// What a screen says when there is nothing to show: what is missing and what
/// to do about it. `action` is the button that does it, when there is one.
pub fn empty_state(
    icon: Lucide,
    title: impl Into<SharedString>,
    hint: impl Into<SharedString>,
    action: Option<AnyElement>,
    cx: &App,
) -> AnyElement {
    v_flex()
        .size_full()
        .items_center()
        .justify_center()
        .gap_2()
        .p_8()
        .child(
            Icon::new(icon)
                .large()
                .text_color(cx.theme().muted_foreground),
        )
        .child(div().font_semibold().child(title.into()))
        .child(
            div()
                .text_sm()
                .text_center()
                .text_color(cx.theme().muted_foreground)
                .child(hint.into()),
        )
        .children(action.map(|action| div().pt_2().child(action)))
        .into_any_element()
}

/// Opens a dialog around a form: its fields, Cancel, and the one commitment
/// Enter makes. `submit` answers whether the form was acceptable; `false`
/// keeps the dialog up so the person can fix what it says is wrong.
///
/// `first` is the field typing should land in. It is focused *after* the
/// dialog opens: the dialog remembers what had the focus before it and gives
/// it back on closing, and that has to be the window behind it, not a field
/// of its own that no longer exists.
pub fn open_form<F: Render>(
    title: impl Into<SharedString>,
    commit: impl Into<SharedString>,
    form: Entity<F>,
    first: Option<FocusHandle>,
    submit: impl Fn(&mut F, &mut Context<F>) -> bool + 'static,
    window: &mut Window,
    cx: &mut App,
) {
    let (title, commit): (SharedString, SharedString) = (title.into(), commit.into());
    let submit = Rc::new(submit);
    window.open_dialog(cx, move |dialog, _, _| {
        let (form, submit) = (form.clone(), submit.clone());
        dialog
            .title(title.clone())
            .child(form.clone())
            .footer(
                DialogFooter::new()
                    .child(DialogClose::new().child(Button::new("cancel").label("Cancel")))
                    .child(
                        DialogAction::new()
                            .child(Button::new("ok").label(commit.clone()).primary()),
                    ),
            )
            .on_ok(move |_, _, cx| form.update(cx, |form, cx| submit(form, cx)))
    });
    if let Some(first) = first {
        first.focus(window, cx);
    }
}

/// Wraps a control that Enter opens or chooses in — a select, a date picker —
/// so that the Enter stops there. The library's select opens its list on
/// Enter and then lets the key go on, and a dialog takes an Enter that
/// reaches it as its commitment: without this, opening the list of
/// categories would also try to save the form.
pub fn choosing(control: impl IntoElement) -> Div {
    div()
        .w_full()
        .on_action(|_: &Confirm, _, _| {})
        .child(control)
}

/// A labelled control, as every form here lays one out.
pub fn field(label: impl Into<SharedString>, control: impl IntoElement) -> Div {
    v_flex()
        .gap_1()
        .child(div().text_sm().font_medium().child(label.into()))
        .child(control)
}

/// What went wrong with a form, said under it.
pub fn form_error(message: impl Into<SharedString>, cx: &App) -> Div {
    div()
        .text_sm()
        .text_color(cx.theme().danger)
        .child(message.into())
}

/// A screen's heading row: what it is, how much of it there is, and whatever
/// the screen puts at its trailing edge.
pub fn heading(
    title: impl Into<SharedString>,
    detail: impl Into<SharedString>,
    trailing: Option<AnyElement>,
    cx: &App,
) -> Div {
    h_flex()
        .flex_none()
        .w_full()
        .gap_3()
        .child(div().text_xl().font_semibold().child(title.into()))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(detail.into()),
        )
        .children(trailing)
}
