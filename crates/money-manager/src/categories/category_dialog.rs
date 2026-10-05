//! The category dialog: a name, a colour and an icon.

use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::color_picker::{ColorPicker, ColorPickerEvent, ColorPickerState};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{ActiveTheme, Colorize, Selectable, StyledExt, h_flex, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::{App, AppContext, Context, Entity, Hsla, SharedString, Subscription, Window, div};

use crate::book::{self, Book, CategoryInfo, CategoryLook, CategoryStyle, ICONS, Mode, PALETTE};
use crate::ui::{self, Lucide, TileSize};

struct CategoryForm {
    book: Entity<Book>,
    mode: Mode,
    /// The category being changed, or nothing for a new one.
    editing: Option<CategoryInfo>,
    name: Entity<InputState>,
    color: Entity<ColorPickerState>,
    /// The colour as it will be stored, or nothing to keep deriving it from
    /// the name.
    chosen_color: Option<String>,
    /// A key of [`ICONS`], or nothing for the default icon.
    chosen_icon: Option<String>,
    error: Option<SharedString>,
    _subscriptions: Vec<Subscription>,
}

/// `#rrggbb` of an opaque colour, which is the only form that is stored.
fn stored_color(color: Hsla) -> String {
    Hsla { a: 1.0, ..color }.to_hex().to_lowercase()
}

impl CategoryForm {
    fn new(
        book: Entity<Book>,
        mode: Mode,
        editing: Option<CategoryInfo>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let style = editing
            .as_ref()
            .map(|category| category.style.clone())
            .unwrap_or_default();
        let name = cx.new(|cx| {
            let state = InputState::new(window, cx).placeholder("Groceries");
            match &editing {
                Some(category) => state.default_value(category.name.clone()),
                None => state,
            }
        });
        // The picker holds a colour only once one was chosen: until then
        // the colour follows the name, and the preview beside it shows it.
        let chosen = style
            .color
            .as_deref()
            .and_then(|stored| Hsla::parse_hex(stored).ok());
        let color = cx.new(|cx| {
            let state = ColorPickerState::new(window, cx);
            match chosen {
                Some(chosen) => state.default_value(chosen),
                None => state,
            }
        });
        let subscriptions = vec![
            cx.subscribe(&color, |this, _, event: &ColorPickerEvent, cx| {
                let ColorPickerEvent::Change(picked) = event;
                this.chosen_color = picked.map(stored_color);
                cx.notify();
            }),
            // The preview is drawn from the name while no colour is chosen.
            cx.subscribe(&name, |_, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
            }),
        ];
        CategoryForm {
            book,
            mode,
            editing,
            name,
            color,
            chosen_color: style.color,
            chosen_icon: style.icon,
            error: None,
            _subscriptions: subscriptions,
        }
    }

    /// How the category will be drawn if saved as the form stands.
    fn look(&self, cx: &App) -> CategoryLook {
        let name = self.name.read(cx).value();
        let color = self
            .chosen_color
            .as_deref()
            .and_then(|stored| Hsla::parse_hex(stored).ok())
            .unwrap_or_else(|| {
                // No id yet for a new category: an empty one derives from
                // the name, exactly as the saved category will.
                book::color_of(
                    &CategoryStyle::default(),
                    &[],
                    &name,
                    cx.theme().muted_foreground,
                )
            });
        let icon = self
            .chosen_icon
            .as_deref()
            .and_then(|key| ICONS.iter().find(|(name, _)| *name == key))
            .map_or(Lucide::Tag, |(_, icon)| *icon);
        CategoryLook { color, icon }
    }

    /// Saves the category — name, colour and icon in one record. Answers
    /// whether it did.
    fn submit(&mut self, cx: &mut Context<Self>) -> bool {
        let name = self.name.read(cx).value().to_string();
        let (mode, id) = (self.mode, self.editing.as_ref().map(|c| c.id.clone()));
        let style = CategoryStyle {
            color: self.chosen_color.clone(),
            icon: self.chosen_icon.clone(),
        };
        // A category saved as it was is not rewritten: a write is a new
        // stamp, and a stamp that changes nothing would still outrank
        // another device's.
        let unchanged = self.editing.as_ref().is_some_and(|category| {
            category.name.as_ref() == name.trim() && category.style == style
        });
        let saved = if unchanged {
            Ok(())
        } else {
            self.book
                .update(cx, |book, cx| {
                    book.save_category(mode, id.as_deref(), &name, &style, cx)
                })
                .map(drop)
        };
        match saved {
            Ok(()) => true,
            Err(error) => {
                self.error = Some(error.into());
                cx.notify();
                false
            }
        }
    }
}

impl Render for CategoryForm {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let look = self.look(cx);
        let featured: Vec<Hsla> = PALETTE
            .iter()
            .filter_map(|(stored, _)| Hsla::parse_hex(stored).ok())
            .collect();
        let chosen_icon = self.chosen_icon.clone();

        v_flex()
            .gap_4()
            .child(
                h_flex()
                    .gap_3()
                    .child(ui::category_tile(look, TileSize::Preview, cx))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(ui::field("Name", Input::new(&self.name))),
                    ),
            )
            .child(ui::field(
                "Color",
                h_flex()
                    .gap_3()
                    .child(
                        ColorPicker::new(&self.color)
                            .featured_colors(featured)
                            .accessibility_label("Category color"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(match &self.chosen_color {
                                Some(_) => "Chosen",
                                None => "Follows the name until you choose one",
                            }),
                    ),
            ))
            .child(ui::field(
                "Icon",
                div()
                    .grid()
                    .grid_cols(8)
                    .gap_1()
                    .children(ICONS.iter().map(|(key, icon)| {
                        // The default icon is selected by having chosen none.
                        let selected = match &chosen_icon {
                            Some(chosen) => chosen == key,
                            None => *key == "tag",
                        };
                        let key = key.to_string();
                        Button::new(SharedString::from(format!("icon-{key}")))
                            .icon(*icon)
                            .ghost()
                            .selected(selected)
                            .accessibility_label(key.replace('-', " "))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.chosen_icon = Some(key.clone());
                                cx.notify();
                            }))
                    })),
            ))
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .font_normal()
                    .child("Name, color and icon sync with the ledger."),
            )
            .children(self.error.clone().map(|error| ui::form_error(error, cx)))
    }
}

/// Opens the dialog for a new category of `mode`, or for `category`.
pub fn open_category_dialog(
    book: &Entity<Book>,
    mode: Mode,
    category: Option<CategoryInfo>,
    window: &mut Window,
    cx: &mut App,
) {
    let kind = match mode {
        Mode::Expense => "expense",
        Mode::Income => "income",
    };
    let (title, commit) = match &category {
        Some(_) => (format!("Edit {kind} category"), "Save"),
        None => (format!("New {kind} category"), "Add"),
    };
    let form = cx.new(|cx| CategoryForm::new(book.clone(), mode, category, window, cx));
    let name = gpui_kit::Focusable::focus_handle(&form.read(cx).name, cx);
    ui::open_form(
        title,
        commit,
        form,
        Some(name),
        CategoryForm::submit,
        window,
        cx,
    );
}
