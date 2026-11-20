// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The Templates page, under Sending: the templates of every account, a group per account (`[ A ] STUDIO · 4`), a row
//! per template (the account's bar, its name, its subject and text on one line, `edit` and `delete`), "New template" at
//! the title's right end. A delete is confirmed in place, where the row stood: the template's name, the account it goes
//! from, and that it cannot be undone. Edit and New template open the form on the page: the account, the name, the
//! subject and the text, Save and Cancel. Nothing is saved or deleted but on the owner's press of Save or of the
//! confirmation's "Delete template"; a focus, a hover or a preview runs nothing.

use egui::{Align, Id, Key, Layout, Modifiers, Rect, Role, Ui, WidgetInfo, pos2, vec2};

use super::{Asked, body, inset, rule, words};
use crate::component::button::{LinkSize, TextButton, link_sized};
use crate::component::field;
use crate::component::label::section_label;
use crate::component::select::Select;
use crate::iconset::IconId;
use crate::keymap::Keymap;
use crate::mailbox::{AccountRow, Mailbox};
use crate::settings::{Section, Settings};
use crate::templates::{Hand, TemplateRow, Templates};
use crate::theme::Theme;
use crate::tr;

/// A template's row, in points at 100 %.
const ROW: f32 = 48.0;

/// The name's column, in points.
const NAME: f32 = 200.0;

/// Between the row's parts, in points.
const GAP: f32 = 12.0;

/// Between the form's account choice and its name, in points.
const FORM_GAP: f32 = 16.0;

/// Between a row's two links, in points.
const LINK_GAP: f32 = 12.0;

/// The account choice's width beside the name, in points.
const ACCOUNT: f32 = 200.0;

/// Gives the keyboard to `response` when it is where the page asked it to go (`hand`), and takes the ask.
fn take(hand: &mut Option<Hand>, wanted: &Hand, response: &egui::Response) {
    if hand.as_ref() == Some(wanted) {
        *hand = None;
        response.request_focus();
    }
}

/// The page's head: the link back to `back`, the title with `action` at its right end, the sentence under it.
fn head(ui: &mut Ui, theme: &Theme, keymap: &Keymap, back: &str, title: &str, action: impl FnOnce(&mut Ui)) -> bool {
    let p = &theme.palette;
    let mut left = false;
    inset(ui, |ui| {
        let width = ui.available_width();
        ui.allocate_ui_with_layout(vec2(width, super::where_high(theme)), Layout::left_to_right(Align::Center), |ui| {
            left = link_sized(ui, theme, keymap, back, LinkSize::Small).clicked();
        });
        ui.add_space(super::HEAD_GAP);
        let tall = super::title_high(theme);
        ui.allocate_ui_with_layout(vec2(width, tall), Layout::left_to_right(Align::Center), |ui| {
            ui.set_min_height(tall);
            let shown = crate::component::label(ui, theme.fonts.title, title, p.ink);
            shown.widget_info(|| WidgetInfo::labeled(Role::Heading, true, title));
            ui.with_layout(Layout::right_to_left(Align::Center), action);
        });
        ui.add_space(6.0);
        words(
            ui,
            theme.fonts.body,
            tr!(
                "A template is a subject and a text kept on an account; the composer's Templates menu puts one into a draft. {name}, {first_name} and {my_name} are filled in; {?prompt} leaves a blank the draft asks for."
            ),
            p.muted,
        );
    });
    left
}

/// The Templates page.
pub(super) fn page(ui: &mut Ui, theme: &Theme, keymap: &Keymap, settings: &mut Settings, mailbox: &Mailbox) -> Option<Asked> {
    let mut asked = None;
    let templates = settings.templates_mut();
    let mut hand = templates.take_hand();
    if templates.form().is_some() {
        form(ui, theme, keymap, templates, mailbox, &mut hand, &mut asked);
    } else {
        list(ui, theme, keymap, templates, mailbox, &mut hand, &mut asked);
    }
    // A hand to a control not drawn this frame: the name field and "New template" come next frame; a row that went
    // (deleted, out of sight) hands the keyboard to "New template" instead of nowhere.
    match hand {
        Some(Hand::Name) if templates.form().is_some() => templates.hand_to(Hand::Name),
        Some(_) if templates.form().is_none() => templates.hand_to(Hand::New),
        _ => {}
    }
    asked
}

/// The templates, a group per account.
fn list(
    ui: &mut Ui,
    theme: &Theme,
    keymap: &Keymap,
    templates: &mut Templates,
    mailbox: &Mailbox,
    hand: &mut Option<Hand>,
    asked: &mut Option<Asked>,
) {
    let accounts = mailbox.accounts();
    let left = head(ui, theme, keymap, tr!("← Sending"), tr!("Templates"), |ui| {
        let button = TextButton::new(tr!("New template")).secondary();
        let button = if accounts.is_empty() { button.unavailable(tr!("Add an account first")) } else { button };
        let response = button.show(ui, theme, keymap);
        take(hand, &Hand::New, &response);
        if response.clicked()
            && let Some(first) = accounts.first()
        {
            *asked = Some(Asked::NewTemplate(first.id));
        }
    });
    if left {
        *asked = Some(Asked::Section(Section::Sending));
    }
    // Esc with the confirmation open is the confirmation's: the template stays.
    if templates.confirming().is_some() && ui.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Escape)) {
        *asked = Some(Asked::KeepTemplate);
    }
    if let Some(note) = templates.note() {
        ui.add_space(12.0);
        inset(ui, |ui| words(ui, theme.fonts.body, note, theme.palette.warn));
    }
    body(ui, theme, "templates", |ui| {
        if templates.rows().is_empty() {
            ui.add_space(super::GROUP_TOP);
            crate::component::empty::empty_state(
                ui,
                theme,
                tr!("Templates · none"),
                tr!("No templates yet."),
                tr!("Keep a text you write often, with its subject. The composer's Templates menu then offers it in every new message."),
            );
            return;
        }
        let total = templates.rows().len();
        let mut at = 0_usize;
        let mut letter = 0_u32;
        while let Some(first) = templates.rows().get(at) {
            let account = first.account;
            let run = templates.rows().get(at..).unwrap_or_default().iter().take_while(|row| row.account == account).count().max(1);
            let label = accounts.iter().find(|row| row.id == account);
            let name = label.map_or_else(|| tr!("Account {id}", id = account), |row| crate::component::plain(&row.label).into_owned());
            let index = format!("[ {} ]", group_letter(letter));
            ui.add_space(super::GROUP_TOP);
            inset(ui, |ui| section_label(ui, theme, &index, &format!("{name} · {run}"), false));
            ui.add_space(super::GROUP_UNDER);
            rule(ui, theme);
            for offset in 0..run {
                let shown = at.saturating_add(offset);
                let ends = shown == 0 || shown.saturating_add(1) == total;
                if let Some(row) = templates.rows().get(shown) {
                    let confirming = templates.confirming().is_some_and(|(account, name)| account == row.account && name == row.name);
                    if let Some(pressed) = template_row(ui, theme, keymap, row, label, confirming, ends, hand) {
                        *asked = Some(pressed);
                    }
                }
            }
            at = at.saturating_add(run);
            letter = letter.saturating_add(1);
        }
    });
}

/// The letter of the `n`th group, from A; past Z the groups count on.
fn group_letter(n: u32) -> String {
    char::from_u32(u32::from(b'A').saturating_add(n)).filter(char::is_ascii_uppercase).map_or_else(|| n.saturating_add(1).to_string(), String::from)
}

/// A template's row, or its confirmation in its place; drawn only near the view unless it `ends` the list.
#[allow(clippy::too_many_arguments, reason = "the row's parts and the page's hand, each its own")]
fn template_row(
    ui: &mut Ui,
    theme: &Theme,
    keymap: &Keymap,
    row: &TemplateRow,
    account: Option<&AccountRow>,
    confirming: bool,
    ends: bool,
    hand: &mut Option<Hand>,
) -> Option<Asked> {
    if confirming {
        let own = egui::UiBuilder::new().scope_id(Id::unique("settings-template-confirm"));
        return ui.scope_builder(own, |ui| confirmation(ui, theme, keymap, row, account, hand)).inner;
    }
    let width = ui.available_width();
    let high = ROW.max(theme.fonts.body.line + 26.0).round();
    let top = ui.cursor().top();
    let rect = Rect::from_min_size(pos2(ui.max_rect().left(), top), vec2(width, high));
    let clip = ui.clip_rect();
    if !ends && !clip.expand2(vec2(0.0, clip.height())).intersects(rect) {
        ui.allocate_space(vec2(width, high + 1.0));
        return None;
    }
    let p = &theme.palette;
    let mut asked = None;
    // The row's own scope, so rows above it laid out or passed over as the page scrolls never change its links' ids: the
    // focus stays on them.
    let own = egui::UiBuilder::new()
        .scope_id(Id::unique("settings-template").with((row.account, &row.name)))
        .max_rect(Rect::from_min_size(pos2(ui.max_rect().left(), top), vec2(width, high)))
        .layout(Layout::left_to_right(Align::Center));
    ui.scope_builder(own, |ui| {
        ui.set_min_height(high);
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.add_space(16.0);
        let _ = crate::component::account_bar(ui, theme, account.map_or(0, |row| u64::try_from(row.index).unwrap_or(0)));
        ui.add_space(12.0);
        let name = crate::component::cut(ui, &row.shown, theme.fonts.strong, p.ink, NAME);
        let (place, _) = ui.allocate_exact_size(vec2(NAME, high), egui::Sense::hover());
        ui.painter().galley(pos2(place.left(), place.center().y - name.size().y / 2.0), name, p.ink);
        ui.add_space(GAP);
        // The links' room first, so the line can be cut to what is left and the keyboard meets edit before delete.
        let (edit_word, delete_word) = (tr!("edit"), tr!("delete"));
        let font = theme.fonts.mono_s;
        let links = crate::component::galley(ui, edit_word, font, p.ink).size().x.ceil()
            + crate::component::galley(ui, delete_word, font, p.ink).size().x.ceil()
            + LINK_GAP
            + 16.0;
        let room = (ui.available_width() - links - GAP).max(1.0);
        let line = crate::component::cut(ui, &row.line, theme.fonts.body, p.muted, room);
        let (place, _) = ui.allocate_exact_size(vec2(room, high), egui::Sense::hover());
        ui.painter().galley(pos2(place.left(), place.center().y - line.size().y / 2.0), line, p.muted);
        ui.add_space(GAP);
        let edit = link_sized(ui, theme, keymap, edit_word, LinkSize::Small);
        let spoken = tr!("Edit the template {name}", name = row.shown);
        ui.ctx().accesskit_node_builder(edit.id, |node| node.set_label(spoken.as_str()));
        ui.add_space(LINK_GAP);
        let delete = link_sized(ui, theme, keymap, delete_word, LinkSize::Small);
        take(hand, &Hand::Delete(row.account, row.name.clone()), &delete);
        let spoken = tr!("Delete the template {name}", name = row.shown);
        ui.ctx().accesskit_node_builder(delete.id, |node| node.set_label(spoken.as_str()));
        if edit.clicked() {
            asked = Some(Asked::EditTemplate(row.account, row.name.clone()));
        }
        if crate::component::button::owner_pressed(ui, &delete) {
            asked = Some(Asked::AskDeleteTemplate(row.account, row.name.clone()));
        }
    });
    rule(ui, theme);
    asked
}

/// The confirmation of a delete, in the row's place: what goes, from where, that it cannot be undone, Cancel and
/// "Delete template".
fn confirmation(
    ui: &mut Ui,
    theme: &Theme,
    keymap: &Keymap,
    row: &TemplateRow,
    account: Option<&AccountRow>,
    hand: &mut Option<Hand>,
) -> Option<Asked> {
    let p = &theme.palette;
    let mut asked = None;
    let from = account.map_or_else(|| tr!("its account").to_owned(), |account| crate::component::plain(&account.label).into_owned());
    ui.add_space(16.0);
    inset(ui, |ui| {
        let question = tr!("Delete the template “{name}”?", name = row.shown);
        let heading = crate::component::label(ui, theme.fonts.heading, &question, p.ink);
        heading.widget_info(|| WidgetInfo::labeled(Role::Heading, true, &question));
        ui.add_space(8.0);
        words(ui, theme.fonts.body, tr!("This cannot be undone."), p.ink);
        words(ui, theme.fonts.body, &tr!("It goes from {account}.", account = from), p.ink);
        words(ui, theme.fonts.body, tr!("Drafts and sent messages that were written with it keep their text."), p.muted);
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 12.0;
            let cancel = TextButton::new(tr!("Cancel")).secondary().show(ui, theme, keymap);
            take(hand, &Hand::Cancel, &cancel);
            let delete = TextButton::new(tr!("Delete template")).danger().show(ui, theme, keymap);
            if cancel.clicked() {
                asked = Some(Asked::KeepTemplate);
            }
            if crate::component::button::owner_pressed(ui, &delete) {
                asked = Some(Asked::DeleteTemplate(row.account, row.name.clone()));
            }
        });
    });
    ui.add_space(16.0);
    rule(ui, theme);
    asked
}

/// The form: the account and the name side by side, the subject, the text; Save and Cancel.
fn form(
    ui: &mut Ui,
    theme: &Theme,
    keymap: &Keymap,
    templates: &mut Templates,
    mailbox: &Mailbox,
    hand: &mut Option<Hand>,
    asked: &mut Option<Asked>,
) {
    let accounts = mailbox.accounts();
    let replaces = templates.replaces();
    let note = templates.note().map(str::to_owned);
    let editing = templates.form().is_some_and(|form| form.was.is_some());
    let title = if editing { tr!("Edit a template") } else { tr!("New template") };
    if head(ui, theme, keymap, tr!("← Templates"), title, |_| {}) {
        *asked = Some(Asked::CloseTemplate);
    }
    let mut choosing = *templates.choosing_mut();
    let Some(form) = templates.form_mut() else { return };
    body(ui, theme, "template-form", |ui| {
        ui.add_space(super::GROUP_TOP);
        inset(ui, |ui| {
            let width = ui.available_width();
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = FORM_GAP;
                ui.allocate_ui_with_layout(vec2(ACCOUNT, 0.0), Layout::top_down(Align::Min), |ui| {
                    ui.spacing_mut().item_spacing.y = 8.0;
                    crate::component::label(ui, theme.fonts.label, tr!("Account"), theme.palette.muted);
                    let names: Vec<String> = accounts.iter().map(|row| crate::component::plain(&row.label).into_owned()).collect();
                    let options: Vec<(u64, &str)> = accounts.iter().zip(&names).map(|(row, name)| (row.id, name.as_str())).collect();
                    let shown = options.iter().find(|(id, _)| *id == form.account).map_or("", |(_, name)| *name);
                    let select =
                        Select { name: tr!("Account"), shown, options: &options, current: Some(form.account), icon: IconId::Inbox, width: ACCOUNT };
                    let (_, chosen) = select.show(ui, theme, &mut choosing);
                    if let Some(chosen) = chosen {
                        form.account = chosen;
                    }
                });
                let rest = (width - ACCOUNT - FORM_GAP).max(1.0);
                ui.allocate_ui_with_layout(vec2(rest, 0.0), Layout::top_down(Align::Min), |ui| {
                    ui.set_width(rest);
                    let field = field::Field::new(tr!("Name"), tr!("What the Templates menu shows"));
                    let name = field::text_field_limited(
                        ui,
                        theme,
                        Id::unique("templates-name"),
                        &mut form.name,
                        &field,
                        inboxcraft_model::Template::MAX_NAME_CHARS,
                    );
                    take(hand, &Hand::Name, &name);
                });
            });
            ui.add_space(12.0);
            let field = field::Field::new(tr!("Subject"), tr!("optional"));
            field::text_field_limited(
                ui,
                theme,
                Id::unique("templates-subject"),
                &mut form.subject,
                &field,
                inboxcraft_model::Template::MAX_SUBJECT_CHARS,
            );
            ui.add_space(12.0);
            let field = field::Field::new(tr!("Text"), tr!("Hello {first_name}, …"));
            field::multiline_field_holding(
                ui,
                theme,
                Id::unique("templates-text"),
                &mut form.text,
                &field,
                field::Cap::Chars(inboxcraft_model::Template::MAX_TEXT_CHARS),
            );
            if replaces {
                ui.add_space(8.0);
                let account = accounts
                    .iter()
                    .find(|row| row.id == form.account)
                    .map_or_else(String::new, |row| crate::component::plain(&row.label).into_owned());
                let name = crate::component::plain(form.name.trim()).into_owned();
                words(
                    ui,
                    theme.fonts.caption,
                    &tr!("{account} has a template named “{name}”: Save replaces it.", account = account, name = name),
                    theme.palette.muted,
                );
            }
            if let Some(note) = &note {
                ui.add_space(8.0);
                words(ui, theme.fonts.body, note, theme.palette.warn);
            }
            ui.add_space(16.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 12.0;
                let named = !form.name.trim().is_empty();
                let save = TextButton::new(tr!("Save")).primary();
                let save =
                    if named { save.show(ui, theme, keymap) } else { save.unavailable(tr!("A template needs a name")).show(ui, theme, keymap) };
                if named && crate::component::button::owner_pressed(ui, &save) {
                    *asked = Some(Asked::SaveTemplate);
                }
                if TextButton::new(tr!("Cancel")).plain().show(ui, theme, keymap).clicked() {
                    *asked = Some(Asked::CloseTemplate);
                }
            });
        });
    });
    *templates.choosing_mut() = choosing;
}

/// What the Templates page asked for, done: each change through its command, on the owner's press only.
impl super::super::InboxApp {
    /// Shows the Templates page with the templates `template.list` answers now.
    pub fn open_templates(&mut self) {
        self.settings.open_page(Some(crate::settings::Under::Templates.id()));
        self.load_templates();
    }

    /// Reads the templates again; a refusal leaves none and says why on the page.
    fn load_templates(&mut self) {
        match self.mailbox.run("template.list", &serde_json::json!({})) {
            Ok(answer) => self.settings.templates_mut().read(&answer),
            Err(error) => {
                self.settings.templates_mut().read(&serde_json::Value::Null);
                self.settings.templates_mut().set_note(Some(error.to_string()));
            }
        }
    }

    /// Does what the Templates page asked for.
    pub(in crate::draw) fn apply_templates(&mut self, asked: Asked) {
        match asked {
            Asked::Templates => self.open_templates(),
            Asked::NewTemplate(account) => {
                self.settings.templates_mut().write_new(account);
                self.settings.templates_mut().hand_to(Hand::Name);
            }
            Asked::EditTemplate(account, name) => {
                self.settings.templates_mut().edit(account, &name);
                self.settings.templates_mut().hand_to(Hand::Name);
            }
            Asked::CloseTemplate => {
                self.settings.templates_mut().close_form();
                self.settings.templates_mut().hand_to(Hand::New);
            }
            Asked::SaveTemplate => self.save_template(),
            Asked::AskDeleteTemplate(account, name) => {
                self.settings.templates_mut().ask_delete(account, &name);
                self.settings.templates_mut().hand_to(Hand::Cancel);
            }
            Asked::KeepTemplate => {
                if let Some((account, name)) = self.settings.templates().confirming().map(|(account, name)| (account, name.to_owned())) {
                    self.settings.templates_mut().hand_to(Hand::Delete(account, name));
                }
                self.settings.templates_mut().keep();
            }
            Asked::DeleteTemplate(account, name) => {
                let deleted = self.mailbox.run("template.delete", &serde_json::json!({ "account": account, "name": name }));
                self.settings.templates_mut().keep();
                self.load_templates();
                if let Err(error) = deleted {
                    self.settings.templates_mut().set_note(Some(error.to_string()));
                }
                self.settings.templates_mut().hand_to(Hand::New);
            }
            _ => {}
        }
    }

    /// Saves the form's template (`template.save`); a renamed or moved one leaves its old self (`template.delete`)
    /// only once the new one is kept. A refusal keeps the form as typed and says why.
    fn save_template(&mut self) {
        let Some(form) = self.settings.templates().form().cloned() else { return };
        if let Err(error) = self.mailbox.run("template.save", &form.params()) {
            self.settings.templates_mut().set_note(Some(error.to_string()));
            return;
        }
        let left = form.leaves().map(|(account, name)| self.mailbox.run("template.delete", &serde_json::json!({ "account": account, "name": name })));
        self.settings.templates_mut().close_form();
        self.load_templates();
        if let Some(Err(error)) = left {
            self.settings.templates_mut().set_note(Some(error.to_string()));
        }
        self.settings.templates_mut().hand_to(Hand::New);
    }
}

#[cfg(test)]
#[path = "settings_templates_tests.rs"]
mod tests;
