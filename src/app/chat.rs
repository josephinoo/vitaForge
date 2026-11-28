//! Navigation state of the Messages screen. The network side lives in `data::chat`.

use crate::data::chat::{Client, Command, Conversation, Snapshot};
use crate::input::{AppCommand, InputCommand};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Prompt {
    Direct,
    NewGroup,
    Join,
    Message,
    Edit(i64, String),
    AddMember,
    RemoveMember,
    Rename,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Row {
    NewDirect,
    NewGroup,
    Join,
    Conversation(i64),
    /// Index into the open thread's lines.
    Line(usize),
    Write,
    Invite,
    AddMember,
    RemoveMember,
    Rename,
    Leave,
    Block,
    Unblock,
}

#[derive(Default)]
pub struct ChatScreen {
    /// Conversation the user is in; the thread itself may still be loading.
    pub open: Option<i64>,
    pub focus: usize,
    pub prompt: Option<Prompt>,
    /// Leave and block need a second press.
    pub armed: Option<Row>,
}

/// The focused pane's rows: the conversation list, or the open thread's.
pub fn rows(open: Option<i64>, snapshot: &Snapshot) -> Vec<Row> {
    let Some(open) = open else {
        let mut rows = vec![Row::NewDirect, Row::NewGroup, Row::Join];
        rows.extend(snapshot.inbox.iter().map(|c| Row::Conversation(c.id)));
        return rows;
    };
    // -1: a conversation being created, whose id isn't known yet.
    let Some(thread) = snapshot.thread.as_ref().filter(|t| open < 0 || t.conversation.id == open) else { return Vec::new() };
    let mut rows: Vec<Row> = (0..thread.lines.len()).map(Row::Line).collect();
    rows.push(Row::Write);
    rows.extend(actions(&thread.conversation));
    rows
}

fn actions(conversation: &Conversation) -> Vec<Row> {
    match (conversation.is_group(), conversation.role == "owner") {
        (true, true) => vec![Row::Invite, Row::AddMember, Row::RemoveMember, Row::Rename, Row::Leave],
        (true, false) => vec![Row::Leave],
        (false, _) => vec![Row::Block, Row::Unblock],
    }
}

impl ChatScreen {
    /// Focus is clamped here so rows arriving or leaving can't strand it. A thread
    /// opens on "Write" (`usize::MAX` means "not placed yet").
    pub fn focused(&self, rows: &[Row]) -> usize {
        if self.focus == usize::MAX {
            return rows.iter().position(|r| *r == Row::Write).unwrap_or(0);
        }
        self.focus.min(rows.len().saturating_sub(1))
    }

    /// Returns false when the screen should close.
    pub fn handle(&mut self, command: AppCommand, client: &Client) -> bool {
        if let Some(prompt) = &self.prompt {
            match command {
                AppCommand::SubmitChatText(text) => {
                    let text = text.trim().to_owned();
                    let prompt = prompt.clone();
                    self.prompt = None;
                    if !text.is_empty() {
                        self.submit(prompt, text, client);
                    }
                }
                AppCommand::CloseChatEntry => self.prompt = None,
                _ => {}
            }
            return true;
        }
        let rows = rows(self.open, &client.snapshot());
        let focus = self.focused(&rows);
        match command {
            AppCommand::Input(InputCommand::MoveUp) => self.focus = focus.saturating_sub(1),
            AppCommand::Input(InputCommand::MoveDown) => self.focus = (focus + 1).min(rows.len().saturating_sub(1)),
            AppCommand::Input(InputCommand::Confirm) => {
                if let Some(&row) = rows.get(focus) {
                    self.activate(row, client);
                }
            }
            AppCommand::ChatActivate(row) => {
                if let Some(index) = rows.iter().position(|r| *r == row) {
                    self.focus = index;
                }
                self.activate(row, client);
            }
            AppCommand::CloseChat => return false,
            AppCommand::Input(InputCommand::Back) if self.open.is_none() => return false,
            AppCommand::Input(InputCommand::Back | InputCommand::MoveLeft) => {
                self.armed = None;
                if let Some(id) = self.open.take() {
                    // Back onto the conversation just left.
                    let list = self::rows(None, &client.snapshot());
                    self.focus = list.iter().position(|r| *r == Row::Conversation(id)).unwrap_or(0);
                    client.send(Command::CloseThread);
                }
            }
            _ => return true,
        }
        if !matches!(rows.get(self.focus), Some(row) if Some(*row) == self.armed) {
            self.armed = None;
        }
        true
    }

    fn activate(&mut self, row: Row, client: &Client) {
        let confirmed = self.armed == Some(row);
        self.armed = None;
        match row {
            Row::NewDirect => self.prompt = Some(Prompt::Direct),
            Row::NewGroup => self.prompt = Some(Prompt::NewGroup),
            Row::Join => self.prompt = Some(Prompt::Join),
            Row::Conversation(id) => {
                self.enter(id);
                client.send(Command::Open(id));
            }
            Row::Line(index) => {
                let snapshot = client.snapshot();
                let line = snapshot.thread.as_ref().and_then(|t| t.lines.get(index));
                if let Some(line) = line.filter(|l| l.mine)
                    && let Some(text) = &line.text
                {
                    self.prompt = Some(Prompt::Edit(line.id, text.clone()));
                }
            }
            Row::Write => self.prompt = Some(Prompt::Message),
            Row::Invite => client.send(Command::Invite),
            Row::AddMember => self.prompt = Some(Prompt::AddMember),
            Row::RemoveMember => self.prompt = Some(Prompt::RemoveMember),
            Row::Rename => self.prompt = Some(Prompt::Rename),
            Row::Leave | Row::Block if !confirmed => self.armed = Some(row),
            Row::Leave => {
                self.open = None;
                self.focus = 0;
                client.send(Command::Leave);
            }
            Row::Block | Row::Unblock => {
                let other = client.snapshot().thread.as_ref().map(|t| t.conversation.name(&client.username).to_owned());
                if let Some(other) = other {
                    client.send(if row == Row::Block { Command::Block(other) } else { Command::Unblock(other) });
                }
            }
        }
    }

    fn enter(&mut self, id: i64) {
        self.open = Some(id);
        self.focus = usize::MAX;
    }

    fn submit(&mut self, prompt: Prompt, text: String, client: &Client) {
        // The server decides the id of a new or joined chat; -1 shows "loading" until it arrives.
        if matches!(prompt, Prompt::Direct | Prompt::NewGroup | Prompt::Join) {
            self.enter(-1);
        }
        client.send(match prompt {
            Prompt::Direct => Command::Direct(text),
            Prompt::NewGroup => Command::NewGroup(text),
            Prompt::Join => Command::Join(text),
            Prompt::Message => {
                self.focus = usize::MAX;
                Command::Send(text)
            }
            Prompt::Edit(id, _) => Command::Edit(id, text),
            Prompt::AddMember => Command::AddMember(text),
            Prompt::RemoveMember => Command::RemoveMember(text),
            Prompt::Rename => Command::Rename(text),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::chat::Thread;

    #[test]
    fn thread_opens_on_write_and_owner_gets_group_actions() {
        let thread = Thread {
            conversation: Conversation { kind: "group".into(), role: "owner".into(), ..Default::default() },
            ..Default::default()
        };
        let snapshot = Snapshot { thread: Some(thread), ..Default::default() };
        let rows = rows(Some(0), &snapshot);
        let screen = ChatScreen { focus: usize::MAX, ..Default::default() };
        assert_eq!(rows[screen.focused(&rows)], Row::Write);
        assert!(rows.contains(&Row::Invite));
    }
}
