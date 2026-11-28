//! The chat screen: conversations on the left, the open thread on the right.

use crate::app::chat::{ChatScreen, Row, rows};
use crate::app::i18n::Language;
use crate::app::ui::theme::*;
use crate::data::chat::{Client, Line, Notice, Snapshot, Thread};
use crate::input::AppCommand;

const ASIDE_WIDTH: f32 = 230.0;
const AVATAR: f32 = 34.0;

pub(crate) fn chat_page(ctx: &egui::Context, lang: Language, screen: &ChatScreen, client: &Client) -> Vec<AppCommand> {
    let mut commands = Vec::new();
    let snapshot = client.snapshot();
    let thread = snapshot.thread.as_ref().filter(|t| screen.open.is_some_and(|id| id < 0 || t.conversation.id == id));
    let rows = rows(screen.open, &snapshot);
    let focused = rows.get(screen.focused(&rows)).copied();
    // Worker updates arrive without input events.
    ctx.request_repaint_after(std::time::Duration::from_millis(500));

    let panel = |fill| egui::Frame::NONE.fill(fill).inner_margin(8.0);
    egui::SidePanel::left("chat_aside")
        .exact_width(ASIDE_WIDTH)
        .resizable(false)
        .frame(panel(pal().bg_header))
        .show(ctx, |ui| aside(ui, lang, screen, client, &snapshot, focused, &mut commands));
    if let Some(thread) = thread {
        egui::TopBottomPanel::bottom("chat_composer")
            .frame(panel(pal().bg_deep))
            .show(ctx, |ui| composer(ui, lang, screen, thread, &rows, focused, &mut commands));
    }
    egui::CentralPanel::default().frame(panel(pal().bg_deep)).show(ctx, |ui| {
        header(ui, lang, client, &snapshot, thread);
        match thread {
            Some(thread) => messages(ui, lang, thread, focused, &mut commands),
            None => {
                let key = if screen.open.is_some() || !snapshot.loaded { "chat-loading" } else { "chat-pick" };
                ui.centered_and_justified(|ui| ui.label(text(lang.tr(key), FONT_BODY, pal().text_dim)));
            }
        }
    });
    commands
}

fn text(value: impl Into<String>, size: f32, color: egui::Color32) -> egui::RichText {
    egui::RichText::new(value).size(size).color(color)
}

fn avatar(painter: &egui::Painter, center: egui::Pos2, name: &str) {
    painter.circle_filled(center, AVATAR / 2.0, pal().primary);
    let initial = name.chars().next().map_or('?', |c| c.to_ascii_uppercase());
    painter.text(center, egui::Align2::CENTER_CENTER, initial, font(FONT_LARGE), egui::Color32::WHITE);
}

/// Keeps the controller focus on screen, only when it moves.
fn follow_focus(ui: &egui::Ui, response: &egui::Response, key: (Row, usize)) {
    let id = egui::Id::new("chat_focus");
    if ui.data(|d| d.get_temp::<(Row, usize)>(id)) != Some(key) {
        response.scroll_to_me(None);
        ui.data_mut(|d| d.insert_temp(id, key));
    }
}

fn aside(
    ui: &mut egui::Ui,
    lang: Language,
    screen: &ChatScreen,
    client: &Client,
    snapshot: &Snapshot,
    focused: Option<Row>,
    commands: &mut Vec<AppCommand>,
) {
    ui.horizontal(|ui| {
        if ui.button(lang.tr("chat-back")).clicked() {
            commands.push(AppCommand::CloseChat);
        }
        ui.label(text(lang.tr("chat-title"), FONT_TITLE, pal().text).strong());
    });
    ui.add_space(4.0);
    egui::ScrollArea::vertical().id_salt("chat_list").auto_shrink([false, false]).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 4.0;
        let list = crate::app::chat::rows(None, snapshot);
        for &row in &list {
            let conversation = match row {
                Row::Conversation(id) => snapshot.inbox.iter().find(|c| c.id == id),
                _ => None,
            };
            let height = if conversation.is_some() { 50.0 } else { 28.0 };
            let (rect, response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), height), egui::Sense::click());
            let open = matches!(row, Row::Conversation(id) if screen.open == Some(id));
            let is_focused = focused == Some(row);
            if open || is_focused {
                ui.painter().rect_filled(rect, RADIUS_MD, pal().primary.gamma_multiply(0.3));
            }
            if is_focused {
                ui.painter().rect_stroke(rect, RADIUS_MD, egui::Stroke::new(2.0_f32, pal().focus), egui::StrokeKind::Inside);
                follow_focus(ui, &response, (row, list.len()));
            }
            let painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
            if let Some(c) = conversation {
                let name = c.name(&client.username);
                avatar(&painter, egui::pos2(rect.left() + 6.0 + AVATAR / 2.0, rect.center().y), name);
                let left = rect.left() + AVATAR + 14.0;
                let names = painter.with_clip_rect(egui::Rect::from_min_max(rect.min, rect.max - egui::vec2(30.0, 0.0)).intersect(painter.clip_rect()));
                names.text(egui::pos2(left, rect.center().y - 9.0), egui::Align2::LEFT_CENTER, name, font(FONT_BODY), pal().text);
                names.text(egui::pos2(left, rect.center().y + 10.0), egui::Align2::LEFT_CENTER, c.members.join(", "), font(FONT_MICRO), pal().text_dim);
                if c.unread > 0 {
                    let badge = egui::pos2(rect.right() - 16.0, rect.center().y);
                    painter.circle_filled(badge, 10.0, pal().primary);
                    painter.text(badge, egui::Align2::CENTER_CENTER, c.unread.min(99), font(FONT_MICRO), egui::Color32::WHITE);
                }
            } else {
                let key = match row {
                    Row::NewDirect => "chat-new-direct",
                    Row::NewGroup => "chat-new-group",
                    _ => "chat-join",
                };
                painter.text(rect.left_center() + egui::vec2(8.0, 0.0), egui::Align2::LEFT_CENTER, format!("+ {}", lang.tr(key)), font(FONT_SMALL), pal().accent_soft);
            }
            if response.clicked() {
                commands.push(AppCommand::ChatActivate(row));
            }
        }
    });
}

fn header(ui: &mut egui::Ui, lang: Language, client: &Client, snapshot: &Snapshot, thread: Option<&Thread>) {
    let line = |ui: &mut egui::Ui, value: String, color| {
        ui.add(egui::Label::new(text(value, FONT_MICRO, color)).truncate());
    };
    match thread {
        Some(thread) => {
            let name = thread.conversation.name(&client.username);
            ui.horizontal(|ui| {
                let (rect, _) = ui.allocate_exact_size(egui::Vec2::splat(AVATAR), egui::Sense::hover());
                avatar(ui.painter(), rect.center(), name);
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    ui.label(text(name, FONT_TITLE, pal().text).strong());
                    let keys = thread.fingerprints.iter().take(3).map(|(u, f)| format!("{u}: {f}")).collect::<Vec<_>>().join("   ");
                    line(ui, format!("{} {keys}", lang.tr("chat-encrypted")), pal().text_faint);
                });
            });
            if let Some(user) = &thread.key_changed {
                line(ui, lang.tr_args("chat-key-changed", &[("user", user)]), pal().danger);
            }
            if thread.waiting_for_key {
                line(ui, lang.tr("chat-waiting-key"), pal().text_dim);
            }
            ui.separator();
        }
        None if !snapshot.my_fingerprint.is_empty() => {
            line(ui, format!("{} {}", lang.tr("chat-my-key"), snapshot.my_fingerprint), pal().text_faint);
        }
        None => {}
    }
    match &snapshot.notice {
        Some(Notice::Error(error)) => {
            ui.label(text(error.as_str(), FONT_SMALL, pal().danger));
        }
        Some(Notice::InviteCode(code)) => {
            ui.label(text(lang.tr_args("chat-invite-code", &[("code", code)]), FONT_TITLE, pal().text).strong());
        }
        None => {}
    }
}

fn messages(ui: &mut egui::Ui, lang: Language, thread: &Thread, focused: Option<Row>, commands: &mut Vec<AppCommand>) {
    egui::ScrollArea::vertical()
        .id_salt(("chat_thread", thread.conversation.id))
        .auto_shrink([false, false])
        .stick_to_bottom(true)
        .show(ui, |ui| {
            for (index, message) in thread.lines.iter().enumerate() {
                let row = Row::Line(index);
                let response = bubble(ui, lang, message, focused == Some(row));
                if focused == Some(row) {
                    follow_focus(ui, &response, (row, thread.lines.len()));
                }
                if response.clicked() {
                    commands.push(AppCommand::ChatActivate(row));
                }
                ui.add_space(4.0);
            }
        });
}

fn bubble(ui: &mut egui::Ui, lang: Language, message: &Line, focused: bool) -> egui::Response {
    let side = if message.mine { egui::Align::Max } else { egui::Align::Min };
    let layout = if message.mine { egui::Layout::right_to_left(egui::Align::Min) } else { egui::Layout::left_to_right(egui::Align::Min) };
    ui.with_layout(layout, |ui| {
        if !message.mine {
            let (rect, _) = ui.allocate_exact_size(egui::Vec2::splat(AVATAR), egui::Sense::hover());
            avatar(ui.painter(), rect.center(), &message.sender);
        }
        let width = ui.available_width() * 0.75;
        ui.with_layout(egui::Layout::top_down(side), |ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            if !message.mine {
                ui.label(text(message.sender.as_str(), FONT_MICRO, pal().accent_soft));
            }
            let body = message.text.clone().unwrap_or_else(|| lang.tr("chat-unreadable"));
            let color = if message.mine { egui::Color32::WHITE } else { pal().text };
            let pad = egui::vec2(10.0, 6.0);
            let galley = ui.painter().layout(body, font(FONT_BODY), color, width - 2.0 * pad.x);
            let (rect, response) = ui.allocate_exact_size(galley.size() + 2.0 * pad, egui::Sense::click());
            ui.painter().rect_filled(rect, RADIUS_MD, if message.mine { pal().primary } else { pal().bg_card });
            if focused {
                ui.painter().rect_stroke(rect, RADIUS_MD, egui::Stroke::new(2.0_f32, pal().focus), egui::StrokeKind::Outside);
            }
            ui.painter().galley(rect.min + pad, galley, color);
            if message.edited {
                ui.label(text(lang.tr("chat-edited"), FONT_MICRO, pal().text_faint));
            }
            response
        })
        .inner
    })
    .inner
}

fn composer(
    ui: &mut egui::Ui,
    lang: Language,
    screen: &ChatScreen,
    thread: &Thread,
    rows: &[Row],
    focused: Option<Row>,
    commands: &mut Vec<AppCommand>,
) {
    let ring = |on: bool| egui::Stroke::new(if on { 2.0_f32 } else { 1.0 }, if on { pal().focus } else { pal().separator });
    let writing = focused == Some(Row::Write);
    ui.horizontal(|ui| {
        let send_width = 44.0;
        let size = egui::vec2(ui.available_width() - send_width - ui.spacing().item_spacing.x, 36.0);
        let (field, field_response) = ui.allocate_exact_size(size, egui::Sense::click());
        ui.painter().rect_stroke(field, RADIUS_MD, ring(writing), egui::StrokeKind::Inside);
        ui.painter().text(field.left_center() + egui::vec2(12.0, 0.0), egui::Align2::LEFT_CENTER, lang.tr("chat-write"), font(FONT_BODY), pal().text_faint);
        let (send, send_response) = ui.allocate_exact_size(egui::vec2(send_width, 36.0), egui::Sense::click());
        ui.painter().rect_filled(send, RADIUS_MD, pal().primary);
        // A paper plane; the subsetted icon font has none.
        let c = send.center();
        ui.painter().add(egui::Shape::convex_polygon(
            vec![c + egui::vec2(-9.0, -8.0), c + egui::vec2(11.0, 0.0), c + egui::vec2(-9.0, 8.0), c + egui::vec2(-5.0, 0.0)],
            egui::Color32::WHITE,
            egui::Stroke::NONE,
        ));
        if field_response.clicked() || send_response.clicked() {
            commands.push(AppCommand::ChatActivate(Row::Write));
        }
    });
    ui.horizontal_wrapped(|ui| {
        // Everything after "Write" is a conversation action.
        for &row in rows.iter().skip(thread.lines.len() + 1) {
            let key = match row {
                _ if screen.armed == Some(row) => "chat-confirm",
                Row::Invite => "chat-invite",
                Row::AddMember => "chat-add-member",
                Row::RemoveMember => "chat-remove-member",
                Row::Rename => "chat-rename",
                Row::Leave => "chat-leave",
                Row::Block => "chat-block",
                _ => "chat-unblock",
            };
            let button = egui::Button::new(text(lang.tr(key), FONT_MICRO, pal().text)).stroke(ring(focused == Some(row)));
            if ui.add(button).clicked() {
                commands.push(AppCommand::ChatActivate(row));
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::chat::Conversation;

    #[test]
    fn list_and_thread_are_drawn_side_by_side() {
        let ctx = egui::Context::default();
        apply_theme(&ctx);
        let conversation = Conversation { id: 1, kind: "group".into(), title: Some("General".into()), role: "member".into(), ..Default::default() };
        let line = |mine, body: &str| Line { id: 1, sender: "ana".into(), mine, text: Some(body.into()), edited: false };
        let thread = Thread { conversation: conversation.clone(), lines: vec![line(false, "hola"), line(true, "adios")], ..Default::default() };
        let client = Client::fixed("me", Snapshot { loaded: true, inbox: vec![conversation], thread: Some(thread), ..Default::default() });
        let screen = ChatScreen { open: Some(1), focus: usize::MAX, ..Default::default() };
        let mut places = std::collections::HashMap::new();
        for _ in 0..3 {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(738.0, 418.0))),
                ..Default::default()
            };
            let output = ctx.run(input, |ctx| {
                chat_page(ctx, Language::English, &screen, &client);
            });
            for clipped in &output.shapes {
                if let egui::Shape::Text(t) = &clipped.shape {
                    places.insert(t.galley.text().to_owned(), t.pos);
                }
            }
        }
        assert!(places["Messages"].x < ASIDE_WIDTH && places["General"].x > ASIDE_WIDTH, "{places:?}");
        let (other, mine, write) = (places["hola"], places["adios"], places["Write a message"]);
        assert!(other.x > ASIDE_WIDTH && mine.x > other.x + 100.0 && mine.x < 738.0, "{places:?}");
        assert!(write.y > mine.y && write.y < 418.0, "{places:?}");
    }
}
