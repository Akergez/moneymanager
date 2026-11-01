//! The category form: a name, a colour and an icon.

use gpui_adaptive::color_field::{parse_hex, to_hex};
use gpui_adaptive::prim::{key_pressable, row, testable};
use gpui_adaptive::{
    ActiveColors as _, ActiveScale as _, ColorField, ColorFieldEvent, ColorFieldState, FieldEvent,
    Fields, Form, Icon, Labelled, Note, Purpose, TextField, TextFieldState, TileSize, Tone,
};
use gpui_kit::prelude::*;
use gpui_kit::{App, AppContext, Context, Entity, Focusable, SharedString, Subscription, Window, div};

use crate::book::{self, Book, CategoryInfo, CategoryLook, CategoryStyle, ICONS, Mode, PALETTE};
use crate::ui::{self, Lucide};

struct CategoryForm {
    book: Entity<Book>,
    mode: Mode,
    /// The category being changed, or nothing for a new one.
    editing: Option<CategoryInfo>,
    name: Entity<TextFieldState>,
    color: Entity<ColorFieldState>,
    /// The colour as it will be stored, or nothing to keep deriving it from
    /// the name.
    chosen_color: Option<String>,
    /// A key of [`ICONS`], or nothing for the default icon.
    chosen_icon: Option<String>,
    error: Option<SharedString>,
    _subscriptions: Vec<Subscription>,
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
            let mut state = TextFieldState::new(Purpose::Message, window, cx);
            state.set_placeholder("Groceries", window, cx);
            if let Some(category) = &editing {
                state.set_value(category.name.clone(), window, cx);
            }
            state
        });
        // The field holds a colour only once one was chosen: until then the
        // colour follows the name, and the preview beside it shows it.
        let chosen = style.color.as_deref().and_then(parse_hex);
        let color = cx.new(|cx| ColorFieldState::new(chosen, window, cx));
        let subscriptions = vec![
            cx.subscribe(&color, |this, _, event: &ColorFieldEvent, cx| {
                let ColorFieldEvent::Change(picked) = event;
                this.chosen_color = picked.map(to_hex);
                cx.notify();
            }),
            // The preview is drawn from the name while no colour is chosen.
            cx.subscribe(&name, |_, _, event: &FieldEvent, cx| {
                if matches!(event, FieldEvent::Change) {
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
        let name = self.name.read(cx).value(cx);
        let color = self
            .chosen_color
            .as_deref()
            .and_then(parse_hex)
            .unwrap_or_else(|| {
                // No id yet for a new category: an empty one derives from
                // the name, exactly as the saved category will.
                book::color_of(&CategoryStyle::default(), &[], &name, cx.colors().muted)
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
        let name = self.name.read(cx).value(cx).to_string();
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

    /// The icons a category can wear, eight to a row; the one worn marked.
    fn render_icons(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (colors, scale) = (cx.colors(), cx.scale());
        let chosen_icon = self.chosen_icon.clone();
        div()
            .grid()
            .grid_cols(8)
            .gap_1()
            .children(ICONS.iter().map(|(key, icon)| {
                // The default icon is chosen by having chosen none.
                let selected = match &chosen_icon {
                    Some(chosen) => chosen == key,
                    None => *key == "tag",
                };
                let key = key.to_string();
                testable(
                    key_pressable(SharedString::from(format!("icon-{key}")), window, cx)
                        .justify_center()
                        .h(scale.target)
                        .rounded(scale.radius_container)
                        .when(selected, |cell| cell.bg(colors.mine).text_color(colors.on_mine))
                        .child(Icon::new(*icon))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.chosen_icon = Some(key.clone());
                            cx.notify();
                        })),
                )
            }))
    }
}

impl Render for CategoryForm {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let look = self.look(cx);
        let featured = PALETTE.iter().filter_map(|(stored, _)| parse_hex(stored));
        let space = cx.scale().space_m;
        Fields::new()
            .child(
                row()
                    .gap(space)
                    .items_end()
                    .child(ui::category_tile(look, TileSize::Large))
                    .child(div().flex_1().min_w_0().child(TextField::new(&self.name).label("Name"))),
            )
            .child(
                row()
                    .gap(space)
                    .items_end()
                    .child(ColorField::new("category-color", &self.color).label("Color").featured(featured))
                    .child(Note::new(match &self.chosen_color {
                        Some(_) => "Chosen",
                        None => "Follows the name until you choose one",
                    })),
            )
            .child(Labelled::new("Icon", self.render_icons(window, cx)))
            .child(Note::new("Name, color and icon sync with the ledger."))
            .children(
                self.error
                    .clone()
                    .map(|error| Note::new(error).tone(Tone::Danger)),
            )
    }
}

/// Opens the form for a new category of `mode`, or for `category`.
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
    let name = form.read(cx).name.focus_handle(cx);
    let saving = form.clone();
    Form::new(title, form)
        .commit(commit, move |_, cx| saving.update(cx, |form, cx| form.submit(cx)))
        .focus(name)
        .open(window, cx);
}
