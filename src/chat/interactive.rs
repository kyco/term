use anyhow::{anyhow, Result};
use chrono::Local;
use std::collections::VecDeque;
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crossterm::event::{
    DisableBracketedPaste, EnableBracketedPaste, Event, KeyCode, KeyEvent, KeyEventKind,
    KeyModifiers,
};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use crossterm::tty::IsTty;

use crate::branch::BranchService;
use crate::chat::anchor::{
    filter_palette, AnchorRenderer, AnchorState, PaletteItem, SpinnerInfo, StatusInfo,
    SPINNER_FRAMES,
};
use crate::chat::commands::{ChatCommand, InputType};
use crate::chat::editor::{EditorEvent, LineEditor};
use crate::chat::formatter::ChatFormatter;
use crate::chat::state::ChatState;
use crate::config::repository::ConfigRepository;
use crate::config::settings::{ResolvedSettings, SettingsOverrides, SettingsProvider, UserConfig};
use crate::llm::common::model::role::Role;
use crate::path::extract::extract_content;
use crate::path::model::Files;
use crate::repository::db::SqliteRepository;
use crate::session::model::session::Session;
use crate::session::repository::{MessageRepository, SessionRepository};
use crate::session::service::sessions_service;
use crate::ui::timer::ThinkingTimer;
use crate::ui::web_indicator::activity;

/// Legacy per-directory history written by the bottom-anchored UI. Read for
/// migration only; new history goes next to the database.
const LEGACY_HISTORY_FILE: &str = ".termai_history";

/// Input history belongs with the database, not in whichever directory the
/// user happened to `cd` into. The per-directory file meant every project had
/// a different history and none of it was where the user looked for it.
fn history_path() -> Option<std::path::PathBuf> {
    let dir = dirs::config_dir()?.join("termai");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir.join("history"))
}

fn load_history_entries() -> Vec<String> {
    let mut entries: Vec<String> = Vec::new();
    let mut seen = std::collections::HashSet::new();

    // The legacy CWD file first: it is the older half of the timeline.
    let sources = [
        std::fs::read_to_string(LEGACY_HISTORY_FILE).ok(),
        history_path().and_then(|path| std::fs::read_to_string(path).ok()),
    ];
    for content in sources.into_iter().flatten() {
        for line in content.lines() {
            if line.starts_with('#') || line.trim().is_empty() {
                continue;
            }
            if seen.insert(line.to_string()) {
                entries.push(line.to_string());
            }
        }
    }
    entries
}

fn save_history_entries(history: &[String]) {
    if history.is_empty() {
        return;
    }
    if let Some(path) = history_path() {
        let _ = std::fs::write(path, history.join("\n") + "\n");
    }
}

/// Names this tool generated because the user did not supply one.
fn is_auto_name(name: &str) -> bool {
    name == "temporary"
        || name.starts_with("chat-")
        || name.starts_with("chat_")
        || name.starts_with("auto_save_")
}

/// Turn a prompt into a short, readable, greppable session-name fragment.
fn slugify(prompt: &str) -> Option<String> {
    let slug = prompt
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|word| !word.is_empty())
        .take(6)
        .collect::<Vec<_>>()
        .join("-");

    if slug.is_empty() {
        None
    } else {
        Some(slug.chars().take(48).collect())
    }
}

/// SIGHUP (the SSH session or terminal went away) and SIGTERM used to kill
/// the process with the conversation still only in memory.
#[cfg(unix)]
struct Shutdown {
    signals: Option<(
        tokio::signal::unix::Signal,
        tokio::signal::unix::Signal,
    )>,
}

#[cfg(unix)]
impl Shutdown {
    fn new() -> Self {
        use tokio::signal::unix::{signal, SignalKind};
        let signals = match (signal(SignalKind::terminate()), signal(SignalKind::hangup())) {
            (Ok(terminate), Ok(hangup)) => Some((terminate, hangup)),
            _ => None,
        };
        Self { signals }
    }

    async fn recv(&mut self) {
        match self.signals.as_mut() {
            Some((terminate, hangup)) => {
                tokio::select! {
                    _ = terminate.recv() => {}
                    _ = hangup.recv() => {}
                }
            }
            None => std::future::pending::<()>().await,
        }
    }
}

#[cfg(not(unix))]
struct Shutdown;

#[cfg(not(unix))]
impl Shutdown {
    fn new() -> Self {
        Self
    }

    async fn recv(&mut self) {
        std::future::pending::<()>().await
    }
}

/// RAII guard for raw mode + bracketed paste. Always restores the terminal
/// on drop (including panics and error paths).
struct RawModeGuard {
    active: bool,
}

impl RawModeGuard {
    fn new() -> Result<Self> {
        enable_raw_mode()?;
        let _ = crossterm::execute!(std::io::stdout(), EnableBracketedPaste);
        Ok(Self { active: true })
    }

    /// Temporarily restore cooked mode (e.g. while printing formatted output).
    fn suspend(&mut self) {
        if self.active {
            let _ = crossterm::execute!(std::io::stdout(), DisableBracketedPaste);
            let _ = disable_raw_mode();
            self.active = false;
        }
    }

    fn resume(&mut self) {
        if !self.active {
            let _ = enable_raw_mode();
            let _ = crossterm::execute!(std::io::stdout(), EnableBracketedPaste);
            self.active = true;
        }
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        self.suspend();
    }
}

/// Spawn a blocking thread that forwards crossterm events into a tokio
/// channel. The thread polls with a timeout so it can notice shutdown.
fn spawn_input_reader(shutdown: Arc<AtomicBool>) -> tokio::sync::mpsc::UnboundedReceiver<Event> {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    std::thread::spawn(move || loop {
        if shutdown.load(Ordering::SeqCst) {
            break;
        }
        match crossterm::event::poll(Duration::from_millis(100)) {
            Ok(true) => match crossterm::event::read() {
                Ok(ev) => {
                    if tx.send(ev).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            },
            Ok(false) => continue,
            Err(_) => break,
        }
    });
    rx
}

/// Terminal-facing state for the bottom-anchored UI: editor, anchor
/// renderer, palette state and the queue of messages submitted while a
/// response was streaming. Kept separate from `InteractiveSession` so the
/// AI-call future (which mutably borrows the session) never conflicts with
/// UI updates.
struct AnchorUi {
    editor: LineEditor,
    renderer: AnchorRenderer,
    guard: RawModeGuard,
    shutdown: Arc<AtomicBool>,
    queued: VecDeque<String>,
    /// Palette hidden via Esc until the buffer changes again.
    palette_hidden: bool,
    /// Entries pinned while Tab-cycling (so completing doesn't re-filter).
    palette_pin: Option<Vec<PaletteItem>>,
    palette_index: usize,
    /// First Ctrl+C on an empty buffer arms exit; the second exits.
    ctrl_c_armed: bool,
}

impl AnchorUi {
    fn new(guard: RawModeGuard, shutdown: Arc<AtomicBool>) -> Self {
        Self {
            editor: LineEditor::new(),
            renderer: AnchorRenderer::new(),
            guard,
            shutdown,
            queued: VecDeque::new(),
            palette_hidden: false,
            palette_pin: None,
            palette_index: 0,
            ctrl_c_armed: false,
        }
    }

    fn palette_items(&self) -> Vec<PaletteItem> {
        if self.palette_hidden {
            return Vec::new();
        }
        match &self.palette_pin {
            Some(pinned) => pinned.clone(),
            None => filter_palette(self.editor.buffer()),
        }
    }

    fn build_state(&self, status: &StatusInfo, spinner: Option<SpinnerInfo>) -> AnchorState {
        let palette = self.palette_items();
        let selected = if palette.is_empty() {
            0
        } else {
            self.palette_index.min(palette.len() - 1)
        };
        AnchorState {
            input: self.editor.buffer().to_string(),
            cursor: self.editor.cursor(),
            status: status.clone(),
            spinner,
            queued: self.queued.len(),
            palette,
            palette_selected: selected,
        }
    }

    fn draw(&mut self, status: &StatusInfo, spinner: Option<SpinnerInfo>) -> std::io::Result<()> {
        let state = self.build_state(status, spinner);
        let mut out = std::io::stdout();
        self.renderer.draw(&mut out, &state)
    }

    /// Print content into the scrollback above the anchor, then repaint.
    fn print_above(&mut self, content: &str, status: &StatusInfo) -> std::io::Result<()> {
        let state = self.build_state(status, None);
        let mut out = std::io::stdout();
        self.renderer.print_above(&mut out, content, &state)
    }

    /// Erase the anchor and drop to cooked mode so ordinary `println!`-based
    /// output (formatter, command handlers) renders correctly above.
    fn begin_suspended(&mut self) -> std::io::Result<()> {
        let mut out = std::io::stdout();
        self.renderer.erase(&mut out)?;
        self.guard.suspend();
        Ok(())
    }

    fn end_suspended(&mut self) {
        self.guard.resume();
    }

    /// The buffer changed through typing/paste: unpin the palette.
    fn on_buffer_change(&mut self) {
        self.palette_pin = None;
        self.palette_index = 0;
        self.palette_hidden = false;
        self.ctrl_c_armed = false;
    }

    /// Tab: complete to the selected palette entry; further Tabs cycle.
    fn cycle_palette(&mut self) {
        if self.palette_hidden {
            return;
        }
        let items = match &self.palette_pin {
            Some(pinned) => pinned.clone(),
            None => filter_palette(self.editor.buffer()),
        };
        if items.is_empty() {
            return;
        }
        match self.palette_pin {
            None => {
                self.palette_pin = Some(items.clone());
                self.palette_index = 0;
            }
            Some(_) => {
                self.palette_index = (self.palette_index + 1) % items.len();
            }
        }
        self.editor.set_text(&items[self.palette_index].completion);
    }

    /// Tear down: erase the anchor, stop the reader thread, restore cooked
    /// mode. Consumes self so the terminal is clean afterwards.
    fn close(mut self) {
        let mut out = std::io::stdout();
        let _ = self.renderer.erase(&mut out);
        self.shutdown.store(true, Ordering::SeqCst);
        self.guard.suspend();
    }
}

/// Outcome of one anchored AI turn.
enum TurnOutcome {
    Done(Result<()>),
    Cancelled,
    InputClosed,
}

fn is_ctrl_c(key: &KeyEvent) -> bool {
    key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL)
}

/// Manages an interactive chat session.
///
/// On a real TTY this runs the bottom-anchored UI (input pinned at the
/// bottom, conversation flowing into native scrollback, typing allowed while
/// a response is in flight). When stdin or stdout is piped it falls back to
/// a plain line-based loop so scripted/e2e usage keeps working.
pub struct InteractiveSession<'a, R, SR, MR>
where
    R: ConfigRepository,
    SR: SessionRepository,
    MR: MessageRepository,
{
    formatter: ChatFormatter,
    session: Session,
    config_repo: &'a R,
    session_repo: &'a SR,
    message_repo: &'a MR,
    #[allow(dead_code)]
    sqlite_repo: &'a SqliteRepository,
    context_files: Vec<Files>,
    should_exit: bool,
    chat_state: ChatState,
    /// `termai chat "question"` — run as the first turn instead of being
    /// announced and then dropped.
    initial_input: Option<String>,
}

impl<'a, R, SR, MR> InteractiveSession<'a, R, SR, MR>
where
    R: ConfigRepository,
    SR: SessionRepository,
    MR: MessageRepository,
{
    /// Create a new interactive session
    pub fn new(
        config_repo: &'a R,
        session_repo: &'a SR,
        message_repo: &'a MR,
        sqlite_repo: &'a SqliteRepository,
        session: Session,
        context_files: Vec<Files>,
    ) -> Result<Self> {
        let formatter = ChatFormatter::new();

        // Initialize chat state with current provider and model from config
        let chat_state = Self::initialize_chat_state(sqlite_repo)?;

        Ok(Self {
            formatter,
            session,
            config_repo,
            session_repo,
            message_repo,
            sqlite_repo,
            context_files,
            should_exit: false,
            chat_state,
            initial_input: None,
        })
    }

    /// Queue the message given on the command line as the opening turn.
    pub fn with_initial_input(mut self, input: Option<String>) -> Self {
        self.initial_input = input.filter(|text| !text.trim().is_empty());
        self
    }

    /// Start the interactive chat session
    pub async fn run(&mut self) -> Result<()> {
        let interactive_tty = std::io::stdin().is_tty() && std::io::stdout().is_tty();
        if interactive_tty {
            self.run_anchored().await
        } else {
            self.run_plain().await
        }
    }

    /// Print a message to the transcript (cooked-mode paths).
    fn say(&self, message: &str) {
        println!("{}", message);
    }

    // ------------------------------------------------------------------
    // Plain (non-TTY) fallback: read lines from stdin, print responses.
    // ------------------------------------------------------------------

    async fn run_plain(&mut self) -> Result<()> {
        self.display_welcome();
        if !self.context_files.is_empty() {
            self.display_context_info();
        }

        if let Some(initial) = self.initial_input.take() {
            if let Err(e) = self.process_input(&initial).await {
                self.say(&self.formatter.format_error(&e.to_string()));
            }
        }

        use std::io::BufRead;
        loop {
            if self.should_exit {
                break;
            }

            let mut line = String::new();
            let bytes = std::io::stdin().lock().read_line(&mut line)?;
            if bytes == 0 {
                // EOF
                break;
            }
            let input = line.trim_end_matches(['\n', '\r']).to_string();
            if let Err(e) = self.process_input(&input).await {
                self.say(&self.formatter.format_error(&e.to_string()));
            }
        }

        self.finish().await
    }

    /// Save session and print the goodbye message (shared by both modes).
    async fn finish(&mut self) -> Result<()> {
        self.save_on_exit().await?;
        self.say(
            &self
                .formatter
                .format_success("Chat session ended. Goodbye! 👋"),
        );
        Ok(())
    }

    // ------------------------------------------------------------------
    // Bottom-anchored TTY mode
    // ------------------------------------------------------------------

    async fn run_anchored(&mut self) -> Result<()> {
        // In anchored mode the response is painted instantly into scrollback:
        // the typewriter animation would hold the terminal in cooked mode for
        // seconds while the user may be typing their next message.
        self.formatter.set_streaming(false);
        self.formatter.set_show_role_labels(false);

        self.display_welcome();
        if !self.context_files.is_empty() {
            self.display_context_info();
        }

        let guard = RawModeGuard::new()?;
        let shutdown = Arc::new(AtomicBool::new(false));

        let mut rx = spawn_input_reader(shutdown.clone());
        let mut ui = AnchorUi::new(guard, shutdown);

        ui.editor.load_history(load_history_entries());

        activity::set_anchored(true);
        let result = self.anchored_loop(&mut ui, &mut rx).await;
        activity::set_anchored(false);

        let history = ui.editor.history().to_vec();
        ui.close();
        drop(rx);
        save_history_entries(&history);

        // Save first, then report the loop error. Propagating straight out of
        // here used to skip the exit save entirely, so the run that failed was
        // exactly the run whose conversation was thrown away.
        let finished = self.finish().await;
        result?;
        finished
    }

    async fn anchored_loop(
        &mut self,
        ui: &mut AnchorUi,
        rx: &mut tokio::sync::mpsc::UnboundedReceiver<Event>,
    ) -> Result<()> {
        let mut shutdown = Shutdown::new();

        if let Some(initial) = self.initial_input.take() {
            self.process_submission(ui, rx, initial).await?;
        }

        loop {
            if self.should_exit {
                break;
            }

            let status = self.status_info();
            ui.draw(&status, None)?;

            let event = tokio::select! {
                event = rx.recv() => match event {
                    Some(event) => event,
                    None => break,
                },
                _ = shutdown.recv() => {
                    self.should_exit = true;
                    break;
                }
            };

            match event {
                Event::Key(key) => {
                    if key.kind == KeyEventKind::Release {
                        continue;
                    }
                    if key.code == KeyCode::Tab {
                        ui.cycle_palette();
                        continue;
                    }
                    match ui.editor.handle_key(key) {
                        EditorEvent::Submit(text) => {
                            ui.on_buffer_change();
                            self.process_submission(ui, rx, text).await?;
                            // Drain anything queued while the response streamed
                            while !self.should_exit {
                                match ui.queued.pop_front() {
                                    Some(next) => self.process_submission(ui, rx, next).await?,
                                    None => break,
                                }
                            }
                        }
                        EditorEvent::Cancel => {
                            if !ui.palette_items().is_empty() {
                                // Esc closes the palette until the buffer changes
                                ui.palette_hidden = true;
                                ui.palette_pin = None;
                            } else if is_ctrl_c(&key) {
                                if ui.ctrl_c_armed {
                                    self.should_exit = true;
                                } else {
                                    ui.ctrl_c_armed = true;
                                    let status = self.status_info();
                                    ui.print_above(
                                        &self.formatter.format_warning(
                                            "Press Ctrl+C again to exit, or type /exit to quit gracefully",
                                        ),
                                        &status,
                                    )?;
                                }
                            }
                        }
                        EditorEvent::Exit => {
                            self.should_exit = true;
                        }
                        EditorEvent::Redraw => {
                            ui.on_buffer_change();
                        }
                        EditorEvent::None => {}
                    }
                }
                Event::Paste(text) => {
                    ui.editor.insert_str(&text);
                    ui.on_buffer_change();
                }
                Event::Resize(_, _) => {
                    // Repainted at the top of the loop with the new width
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// Handle one submitted line in anchored mode: echo it into scrollback,
    /// then run it as a slash command or an AI turn.
    async fn process_submission(
        &mut self,
        ui: &mut AnchorUi,
        rx: &mut tokio::sync::mpsc::UnboundedReceiver<Event>,
        text: String,
    ) -> Result<()> {
        let text = text.trim().to_string();
        if text.is_empty() {
            return Ok(());
        }

        let status = self.status_info();
        ui.print_above(&format!("\x1b[1;32m  you ›\x1b[0m {}", text), &status)?;

        match InputType::classify(&text) {
            InputType::Command(ChatCommand::Retry) => {
                let mut retry_input: Option<String> = None;
                if self
                    .session
                    .messages
                    .last()
                    .map(|m| m.role == Role::Assistant)
                    .unwrap_or(false)
                {
                    self.session.messages.pop();
                    if let Some(user_msg) = self.session.messages.last() {
                        if user_msg.role == Role::User {
                            retry_input = Some(user_msg.content.clone());
                        }
                    }
                }
                match retry_input {
                    Some(content) => self.anchored_ai_turn(ui, rx, &content).await?,
                    None => {
                        let status = self.status_info();
                        ui.print_above(
                            &self.formatter.format_warning("No AI response to retry"),
                            &status,
                        )?;
                    }
                }
            }
            InputType::Command(command) => {
                // Command handlers print with `println!`: run them in cooked
                // mode with the anchor erased, then repaint.
                ui.begin_suspended()?;
                let result = self.handle_command(command).await;
                ui.end_suspended();
                if let Err(e) = result {
                    let status = self.status_info();
                    ui.print_above(&self.formatter.format_error(&e.to_string()), &status)?;
                }
            }
            InputType::Message(message) => {
                self.session.add_raw_message(message.clone(), Role::User);
                self.anchored_ai_turn(ui, rx, &message).await?;
            }
        }
        Ok(())
    }

    /// Run one AI turn while keeping the anchor alive: the user can keep
    /// typing (their input persists in the anchor), submissions are queued,
    /// and Esc cancels the in-flight request by dropping its future.
    async fn anchored_ai_turn(
        &mut self,
        ui: &mut AnchorUi,
        rx: &mut tokio::sync::mpsc::UnboundedReceiver<Event>,
        user_input: &str,
    ) -> Result<()> {
        // Fold context files into the outgoing message (mirrors plain mode)
        let input_with_context = self.create_contextual_input(user_input);
        if !self.context_files.is_empty() {
            if let Some(last_msg) = self.session.messages.last_mut() {
                if last_msg.role == Role::User {
                    last_msg.content = input_with_context;
                }
            }
        }

        // Store the prompt before the request goes out. If the connection
        // drops, the terminal closes or the process is killed between here
        // and the response, the typed text is already on disk.
        if let Err(e) = sessions_service::write_ahead_user_message(
            self.session_repo,
            self.message_repo,
            &mut self.session,
        ) {
            let status = self.status_info();
            ui.print_above(
                &self
                    .formatter
                    .format_error(&format!("Could not save your message: {}", e)),
                &status,
            )?;
        }

        self.session.redact(self.config_repo);

        // Snapshot status segments: the session is mutably borrowed by the
        // request future below, so the anchor repaints from this copy.
        let status = self.status_info();
        let started = Instant::now();
        let mut ticker = tokio::time::interval(Duration::from_millis(100));
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        let mut shutdown = Shutdown::new();
        let outcome = {
            let fut = Self::call_ai(self.config_repo, &self.chat_state, &mut self.session);
            tokio::pin!(fut);
            loop {
                tokio::select! {
                    result = &mut fut => break TurnOutcome::Done(result),
                    // The terminal went away mid-response. The prompt is
                    // already on disk; leave promptly and save on the way out.
                    _ = shutdown.recv() => break TurnOutcome::InputClosed,
                    _ = ticker.tick() => {
                        let _ = ui.draw(&status, Some(Self::spinner_info(started)));
                    }
                    event = rx.recv() => match event {
                        None => break TurnOutcome::InputClosed,
                        Some(Event::Key(key)) => {
                            if key.kind == KeyEventKind::Release {
                                continue;
                            }
                            match ui.editor.handle_key(key) {
                                EditorEvent::Submit(text) => {
                                    let text = text.trim().to_string();
                                    if !text.is_empty() {
                                        ui.queued.push_back(text);
                                    }
                                }
                                EditorEvent::Cancel => break TurnOutcome::Cancelled,
                                EditorEvent::Exit => {
                                    self.should_exit = true;
                                    break TurnOutcome::Cancelled;
                                }
                                _ => {
                                    let _ = ui.draw(&status, Some(Self::spinner_info(started)));
                                }
                            }
                        }
                        Some(Event::Paste(text)) => {
                            ui.editor.insert_str(&text);
                            let _ = ui.draw(&status, Some(Self::spinner_info(started)));
                        }
                        Some(_) => {}
                    },
                }
            }
        };

        match outcome {
            TurnOutcome::Done(Ok(())) => {
                // Restore the real text before anything is written. The
                // redaction mapping is regenerated per run and never stored,
                // so persisting the placeholder form would corrupt the
                // conversation permanently and unrecoverably.
                self.session.unredact();

                // Store before painting. Rendering drops back to cooked mode,
                // which re-arms Ctrl+C; a save that happened after the paint
                // could lose an answer the user had already read.
                sessions_service::persist_session(
                    self.session_repo,
                    self.message_repo,
                    &mut self.session,
                )?;
                self.name_session_after_first_prompt();

                // Paint the response into scrollback above the anchor.
                ui.begin_suspended()?;
                if let Some(last_message) = self.session.messages.last() {
                    if last_message.role == Role::Assistant {
                        println!("\x1b[1;35m  ai  ›\x1b[0m");
                        let content = last_message.content.clone();
                        if let Err(e) = self
                            .formatter
                            .format_message_async(&Role::Assistant, &content, Some(Local::now()))
                            .await
                        {
                            eprintln!("Error formatting AI response: {}", e);
                            println!("{}", content);
                        }
                        std::io::stdout().flush().ok();
                    }
                }
                ui.end_suspended();
            }
            TurnOutcome::Done(Err(e)) => {
                let unsent = self.take_trailing_user_message();
                let status = self.status_info();
                ui.print_above(
                    &self.formatter.format_error(&format!("AI Error: {}", e)),
                    &status,
                )?;
                self.offer_unsent_back(ui, unsent, &status)?;
            }
            TurnOutcome::Cancelled => {
                // The request future was dropped above, aborting the HTTP
                // call. Take the un-answered user message back out of the
                // conversation, but hand the text back to the editor.
                let unsent = self.take_trailing_user_message();
                let status = self.status_info();
                ui.print_above("\x1b[2m  ✋ response cancelled\x1b[0m", &status)?;
                self.offer_unsent_back(ui, unsent, &status)?;
            }
            TurnOutcome::InputClosed => {
                self.should_exit = true;
            }
        }

        // Failed and cancelled turns still hold redacted text.
        self.session.unredact();
        Ok(())
    }

    /// Remove an unanswered user turn from the conversation and return its
    /// text. The matching row stays on disk marked pending, so the prompt
    /// survives even if the process never gets to hand it back.
    fn take_trailing_user_message(&mut self) -> Option<String> {
        let is_trailing_user = self
            .session
            .messages
            .last()
            .map(|m| m.role == Role::User)
            .unwrap_or(false);
        if !is_trailing_user {
            return None;
        }
        self.session.messages.pop().map(|m| m.content)
    }

    /// Put a failed prompt back in the editor so a dropped connection costs
    /// one keystroke rather than a retyped message.
    fn offer_unsent_back(
        &mut self,
        ui: &mut AnchorUi,
        unsent: Option<String>,
        status: &StatusInfo,
    ) -> Result<()> {
        let Some(text) = unsent else {
            return Ok(());
        };
        if ui.editor.buffer().trim().is_empty() {
            ui.editor.set_text(&text);
            ui.on_buffer_change();
            ui.print_above(
                "\x1b[2m  ↩ your message is back in the prompt — press Enter to resend\x1b[0m",
                status,
            )?;
        } else {
            ui.print_above(
                "\x1b[2m  ↩ your message was saved — recover it with /unsent\x1b[0m",
                status,
            )?;
        }
        Ok(())
    }

    /// Give an auto-named session a name that says what it is about, once
    /// there is a first prompt to derive one from.
    fn name_session_after_first_prompt(&mut self) {
        if !is_auto_name(&self.session.name) {
            return;
        }
        let Some(first_prompt) = self
            .session
            .messages
            .iter()
            .find(|m| m.role == Role::User)
            .map(|m| m.content.clone())
        else {
            return;
        };
        let Some(slug) = slugify(&first_prompt) else {
            return;
        };
        let candidate = format!("{}-{}", Local::now().format("%Y%m%d"), slug);
        // A collision just means the name stays as it is; never fail a turn
        // over cosmetics.
        let _ = sessions_service::rename_session(self.session_repo, &mut self.session, &candidate);
    }

    fn spinner_info(started: Instant) -> SpinnerInfo {
        let frame = (started.elapsed().as_millis() / 100) as usize % SPINNER_FRAMES.len();
        match activity::current() {
            Some((label, secs)) => SpinnerInfo {
                frame,
                elapsed_secs: secs,
                label: format!("🌐 {}", label),
            },
            None => SpinnerInfo {
                frame,
                elapsed_secs: started.elapsed().as_secs_f32(),
                label: "thinking".to_string(),
            },
        }
    }

    fn status_info(&self) -> StatusInfo {
        let effort = self
            .chat_state
            .effective_reasoning_effort(
                crate::config::service::config_service::fetch_reasoning_effort(self.config_repo),
            )
            .map(|effort| effort.to_string());
        StatusInfo {
            model: self.chat_state.model.clone(),
            session: self.session.name.clone(),
            token_estimate: Self::estimate_tokens(&self.session),
            tools_enabled: self.chat_state.tools_enabled,
            effort,
        }
    }

    /// Cheap token estimate (~4 chars per token) for the status line.
    fn estimate_tokens(session: &Session) -> usize {
        session
            .messages
            .iter()
            .map(|m| m.content.chars().count())
            .sum::<usize>()
            / 4
    }

    // ------------------------------------------------------------------
    // Shared input processing (plain mode + command handling)
    // ------------------------------------------------------------------

    /// Process user input (command or message)
    async fn process_input(&mut self, input: &str) -> Result<()> {
        let input = input.trim();
        if input.is_empty() {
            return Ok(());
        }

        match InputType::classify(input) {
            InputType::Command(command) => self.handle_command(command).await,
            InputType::Message(message) => self.handle_message(message).await,
        }
    }

    /// Handle slash commands
    async fn handle_command(&mut self, command: ChatCommand) -> Result<()> {
        match command {
            ChatCommand::Help => {
                let help_text = self.formatter.format_help(&ChatCommand::all_commands());
                self.say(&help_text);
            }
            ChatCommand::Commands => {
                let palette = ChatCommand::command_palette();
                let palette_text = self.formatter.format_command_palette(&palette);
                self.say(&palette_text);
            }
            ChatCommand::Save(name) => {
                self.save_session_as(name)?;
            }
            ChatCommand::Sessions => {
                self.display_recent_sessions();
            }
            ChatCommand::Unsent => {
                self.display_unsent_messages();
            }
            ChatCommand::Context => {
                self.display_context_info();
            }
            ChatCommand::Clear => {
                self.session.messages.clear();
                // Drafts left over from interrupted turns belong to the
                // conversation the user just cleared.
                sessions_service::clear_unsent_messages(self.message_repo, &self.session);
                print!("\x1B[2J\x1B[1;1H"); // Clear screen, home cursor
                std::io::stdout().flush().ok();
                self.display_welcome();
                self.say(&self.formatter.format_conversation_cleared());
            }
            ChatCommand::Exit | ChatCommand::Quit => {
                self.should_exit = true;
            }
            ChatCommand::Retry => {
                if let Some(last_message) = self.session.messages.last() {
                    if last_message.role == Role::Assistant {
                        // Remove the last AI response and regenerate
                        self.session.messages.pop();
                        if let Some(user_message) = self.session.messages.last() {
                            if user_message.role == Role::User {
                                let content = user_message.content.clone();
                                self.generate_ai_response(&content).await?;
                            }
                        }
                    } else {
                        self.say(&self.formatter.format_warning("No AI response to retry"));
                    }
                } else {
                    self.say(
                        &self
                            .formatter
                            .format_warning("No previous message to retry"),
                    );
                }
            }
            ChatCommand::Branch(name) => {
                self.handle_branch_command(name).await?;
            }
            ChatCommand::AddContext(path) => {
                self.add_context_path(&path)?;
            }
            ChatCommand::RemoveContext(path) => {
                self.remove_context_path(&path);
            }
            ChatCommand::Model(model_name) => {
                self.handle_model_command(model_name).await?;
            }
            ChatCommand::Provider(provider_name) => {
                self.handle_provider_command(provider_name).await?;
            }
            ChatCommand::Effort(effort) => {
                self.handle_effort_command(effort);
            }
            ChatCommand::Tools(setting) => {
                self.handle_tools_command(setting);
            }
            ChatCommand::Status => {
                self.say(&self.chat_state.status());
            }
            ChatCommand::Theme(theme_name) => {
                self.handle_theme_command(theme_name);
            }
            ChatCommand::Streaming(setting) => {
                self.handle_streaming_command(setting);
            }
            ChatCommand::Settings => {
                self.display_settings_overview();
            }
        }
        Ok(())
    }

    /// Handle /effort command - show or set the session reasoning effort
    fn handle_effort_command(&mut self, effort: Option<String>) {
        use crate::llm::openai::model::reasoning_effort::{
            sol_only_effort_warning, ReasoningEffort,
        };

        const VALID_VALUES: &str = "none, low, medium, high, xhigh, max, ultra";

        match effort {
            None => {
                let configured = crate::config::service::config_service::fetch_reasoning_effort(
                    self.config_repo,
                );
                let current = match self.chat_state.effective_reasoning_effort(configured) {
                    Some(effort) => effort.to_string(),
                    None => "default (provider default)".to_string(),
                };
                self.say(&format!(
                    "🧠 Reasoning effort: {}\n   Valid values: {} (or 'default' to clear)",
                    current, VALID_VALUES
                ));
            }
            Some(value) => {
                let value = value.to_lowercase();
                if matches!(value.as_str(), "default" | "clear" | "unset") {
                    self.chat_state.set_reasoning_effort(None);
                    self.say(
                        &self
                            .formatter
                            .format_success("Reasoning effort reset to default for this session"),
                    );
                    return;
                }

                match value.parse::<ReasoningEffort>() {
                    Ok(parsed) => {
                        if let Some(warning) =
                            sol_only_effort_warning(&self.chat_state.model, &parsed)
                        {
                            self.say(&self.formatter.format_warning(&warning));
                        }
                        self.chat_state.set_reasoning_effort(Some(parsed.clone()));
                        self.say(&self.formatter.format_success(&format!(
                            "Reasoning effort set to {} for this session",
                            parsed
                        )));
                    }
                    Err(_) => {
                        self.say(&self.formatter.format_warning(&format!(
                            "Invalid reasoning effort '{}'. Valid values: {}",
                            value, VALID_VALUES
                        )));
                    }
                }
            }
        }
    }

    /// Handle /tools command - toggle or set tool usage
    fn handle_tools_command(&mut self, setting: Option<bool>) {
        match setting {
            Some(enabled) => {
                self.chat_state.set_tools_enabled(enabled);
            }
            None => {
                self.chat_state.toggle_tools();
            }
        }

        let status = if self.chat_state.tools_enabled {
            "enabled"
        } else {
            "disabled"
        };

        let provider_note = if self.chat_state.provider != "openai" {
            format!("\n⚠️  Note: Tools are only supported with the OpenAI provider. Current provider: {}", self.chat_state.provider)
        } else {
            String::new()
        };

        self.say(&self.formatter.format_success(&format!(
            "Tools are now {}. The AI can execute bash commands, read/write files, and list directories.{}",
            status,
            provider_note
        )));
    }

    /// Handle regular chat messages (plain mode)
    async fn handle_message(&mut self, message: String) -> Result<()> {
        // Add user message to session
        self.session.add_raw_message(message.clone(), Role::User);

        // Generate AI response
        self.generate_ai_response(&message).await?;

        Ok(())
    }

    /// Generate AI response for the given user input (plain mode)
    async fn generate_ai_response(&mut self, user_input: &str) -> Result<()> {
        // Start thinking timer (no separate message needed)
        let mut timer = ThinkingTimer::new();
        timer.start();

        // Create input with context
        let input_with_context = self.create_contextual_input(user_input);

        // Add context to session
        if !self.context_files.is_empty() {
            // Update the last user message to include context
            if let Some(last_msg) = self.session.messages.last_mut() {
                if last_msg.role == Role::User {
                    last_msg.content = input_with_context;
                }
            }
        }

        // Store the prompt before the request goes out (see anchored mode).
        if let Err(e) = sessions_service::write_ahead_user_message(
            self.session_repo,
            self.message_repo,
            &mut self.session,
        ) {
            self.say(
                &self
                    .formatter
                    .format_error(&format!("Could not save your message: {}", e)),
            );
        }

        // Redact sensitive information
        self.session.redact(self.config_repo);

        // Call AI service based on configured provider
        let result = Self::call_ai(self.config_repo, &self.chat_state, &mut self.session).await;

        timer.stop();

        // Ensure thinking indicator is completely cleared before showing response
        print!("\r\x1b[2K");
        std::io::stdout().flush().unwrap();

        match result {
            Ok(_) => {
                // Real text before storage (see anchored mode), and storage
                // before rendering so a slow paint cannot lose the answer.
                self.session.unredact();
                sessions_service::persist_session(
                    self.session_repo,
                    self.message_repo,
                    &mut self.session,
                )?;
                self.name_session_after_first_prompt();

                // Display AI response with enhanced formatting
                if let Some(last_message) = self.session.messages.last() {
                    if last_message.role == Role::Assistant {
                        // Use the new async formatter for enhanced markdown and syntax highlighting
                        let content = last_message.content.clone();
                        if let Err(e) = self
                            .formatter
                            .format_message_async(&Role::Assistant, &content, Some(Local::now()))
                            .await
                        {
                            eprintln!("Error formatting AI response: {}", e);
                            // Fallback to basic formatting
                            let formatted_ai = self.formatter.format_message(
                                &Role::Assistant,
                                &content,
                                Some(Local::now()),
                            );
                            println!("{}", formatted_ai);
                        }
                        std::io::stdout().flush().unwrap();
                    }
                }
            }
            Err(e) => {
                self.say(&self.formatter.format_error(&format!("AI Error: {}", e)));

                // The prompt stays on disk as an unsent message; drop it from
                // the live conversation so the next turn is not sent twice.
                if self.take_trailing_user_message().is_some() {
                    self.say(&self.formatter.format_warning(
                        "Your message was saved — recover it with /unsent",
                    ));
                }
            }
        }

        // Unredact for display
        self.session.unredact();

        // Ensure we return control properly
        std::io::stdout().flush().unwrap();

        Ok(())
    }

    /// Call the AI service for the configured provider.
    ///
    /// An associated function (not a method) so the anchored UI can poll this
    /// future while separately updating the input line from key events.
    async fn call_ai(config_repo: &R, chat_state: &ChatState, session: &mut Session) -> Result<()> {
        use crate::config::model::keys::ConfigKeys;
        use crate::config::service::config_service;
        use crate::llm::{claude, openai};

        match chat_state.provider.as_str() {
            "claude" => {
                let api_key =
                    config_service::fetch_by_key(config_repo, &ConfigKeys::ClaudeApiKey.to_key())?;
                claude::service::chat::chat_with_model(
                    &api_key.value,
                    session,
                    Some(&chat_state.model),
                )
                .await?;
            }
            "openai" => {
                let api_key =
                    config_service::fetch_by_key(config_repo, &ConfigKeys::ChatGptApiKey.to_key())?;
                if chat_state.tools_enabled {
                    openai::service::chat::chat_with_tools(&api_key.value, session).await?;
                } else {
                    openai::service::chat::chat_with_model(
                        &api_key.value,
                        session,
                        Some(&chat_state.model),
                    )
                    .await?;
                }
            }
            "openai-codex" | "openai_codex" | "codex" => {
                use crate::auth::token_manager::TokenManager;

                // Get valid access token (auto-refreshes if needed)
                let token_manager = TokenManager::new(config_repo);
                let access_token = token_manager
                    .get_valid_token()
                    .await?
                    .ok_or_else(|| anyhow!(
                        "Not authenticated with Codex. Run 'termai auth login codex' to authenticate."
                    ))?;

                // Session /effort override beats the persisted config value.
                let effort = chat_state.effective_reasoning_effort(
                    config_service::fetch_reasoning_effort(config_repo),
                );
                openai::service::codex::chat(
                    &access_token,
                    session,
                    Some(&chat_state.model),
                    effort,
                )
                .await?;
            }
            _ => {
                return Err(anyhow!("Unsupported provider: {}", chat_state.provider));
            }
        }

        Ok(())
    }

    /// Create input with local context
    fn create_contextual_input(&self, user_input: &str) -> String {
        if self.context_files.is_empty() {
            return user_input.to_string();
        }

        let local_context: Vec<String> = self
            .context_files
            .iter()
            .map(|file| format!("{}\n```\n{}```", file.path, file.content))
            .collect();

        format!("{}\n{}", user_input, local_context.join("\n"))
    }

    /// Add a path to the context
    fn add_context_path(&mut self, path: &str) -> Result<()> {
        if !Path::new(path).exists() {
            return Err(anyhow!("Path does not exist: {}", path));
        }

        // Extract content from the path
        let new_context = extract_content(&Some(path.to_string()), &[], &[]);

        if let Some(mut files) = new_context {
            // Remove duplicates and add new files
            for file in files.drain(..) {
                if !self.context_files.iter().any(|f| f.path == file.path) {
                    self.context_files.push(file);
                }
            }
            self.say(
                &self
                    .formatter
                    .format_success(&format!("Added '{}' to context", path)),
            );
            self.display_context_info();
        }

        Ok(())
    }

    /// Remove a path from the context
    fn remove_context_path(&mut self, path: &str) {
        let initial_count = self.context_files.len();
        self.context_files.retain(|file| !file.path.contains(path));

        if self.context_files.len() < initial_count {
            self.say(
                &self
                    .formatter
                    .format_success(&format!("Removed files matching '{}' from context", path)),
            );
            self.display_context_info();
        } else {
            self.say(
                &self
                    .formatter
                    .format_warning(&format!("No files matching '{}' found in context", path)),
            );
        }
    }

    /// Display welcome message
    fn display_welcome(&self) {
        println!(); // Add spacing before welcome
        self.say(&self.formatter.format_welcome());
        println!(); // Add spacing after welcome
    }

    /// Display current context information
    fn display_context_info(&self) {
        let file_paths: Vec<String> = self.context_files.iter().map(|f| f.path.clone()).collect();
        let context_info = self
            .formatter
            .format_context_info(file_paths.len(), &file_paths);
        self.say(&context_info);
    }

    /// Final safety net on the way out. Every turn is already written as it
    /// completes; this catches anything added since and tells the user, from
    /// the database rather than from hope, where the conversation lives.
    async fn save_on_exit(&mut self) -> Result<()> {
        if self.session.messages.is_empty() {
            // Opening a chat and closing it again should not leave a session
            // behind for the user to wade through later.
            sessions_service::discard_if_empty(self.session_repo, self.message_repo, &self.session);
            return Ok(());
        }

        if self.session.temporary {
            self.say(&self.formatter.format_warning(
                "This was a --temporary chat, so nothing was saved. Use /save <name> next time to keep it.",
            ));
            return Ok(());
        }

        self.name_session_after_first_prompt();
        sessions_service::persist_session(
            self.session_repo,
            self.message_repo,
            &mut self.session,
        )?;

        match sessions_service::stored_message_count(self.message_repo, &self.session) {
            Ok(stored) if stored > 0 => self.say(&self.formatter.format_success(&format!(
                "Saved {} messages to '{}' — resume with: termai chat --session {}",
                stored, self.session.name, self.session.name
            ))),
            Ok(_) => self.say(&self.formatter.format_error(
                "Nothing reached the database — this conversation was NOT saved.",
            )),
            Err(e) => self.say(
                &self
                    .formatter
                    .format_error(&format!("Could not verify the save: {}", e)),
            ),
        }

        Ok(())
    }

    /// `/save [name]`: name the conversation, then confirm from the database
    /// that it is really there before claiming it was saved.
    fn save_session_as(&mut self, name: Option<String>) -> Result<()> {
        // `/save` on a `--temporary` chat is an explicit request to keep it.
        let was_temporary = self.session.temporary;
        self.session.temporary = false;

        let target = name
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());

        if let Some(target) = target {
            if let Err(e) =
                sessions_service::rename_session(self.session_repo, &mut self.session, &target)
            {
                self.session.temporary = was_temporary;
                self.say(&self.formatter.format_error(&e.to_string()));
                return Ok(());
            }
        } else {
            self.name_session_after_first_prompt();
        }

        sessions_service::persist_session(
            self.session_repo,
            self.message_repo,
            &mut self.session,
        )?;

        let stored = sessions_service::stored_message_count(self.message_repo, &self.session)?;
        if stored == 0 && !self.session.messages.is_empty() {
            self.say(&self.formatter.format_error(
                "Nothing was written to the database — this session was NOT saved.",
            ));
            return Ok(());
        }

        self.say(&self.formatter.format_session_saved(&self.session.name));
        self.say(&self.formatter.format_success(&format!(
            "{} messages stored — resume with: termai chat --session {}",
            stored, self.session.name
        )));
        Ok(())
    }

    /// `/sessions`: the recent conversations, without leaving the chat.
    fn display_recent_sessions(&self) {
        if let Err(e) = sessions_service::fetch_sessions_with_options(
            self.session_repo,
            self.message_repo,
            None,
            Some(10),
            &crate::args::SessionSortOrder::Date,
        ) {
            self.say(
                &self
                    .formatter
                    .format_error(&format!("Could not list sessions: {}", e)),
            );
        }
    }

    /// `/unsent`: prompts that were stored but never answered.
    fn display_unsent_messages(&self) {
        let unsent = sessions_service::recover_unsent_messages(self.message_repo, &self.session);
        if unsent.is_empty() {
            self.say(
                &self
                    .formatter
                    .format_success("No unsent messages — everything you typed got a reply."),
            );
            return;
        }
        self.say(&self.formatter.format_warning(&format!(
            "{} message(s) never got a reply:",
            unsent.len()
        )));
        for message in &unsent {
            self.say(&format!("\n{}\n", message));
        }
    }

    /// Handle the /branch command
    async fn handle_branch_command(&mut self, name: Option<String>) -> Result<()> {
        // Generate branch name with context hint
        let branch_name = if let Some(name) = name.clone() {
            name
        } else {
            BranchService::generate_branch_name(&self.session.id, None)
        };

        // Branching needs a &mut repository and this session only holds a
        // shared one, so in-chat /branch cannot create anything. Say that
        // plainly and point at the command that works, rather than printing a
        // branch name the user will look for later and never find.
        let _ = &branch_name;
        let message = format!(
            "🌿 /branch is not available inside chat yet.\n   Save this conversation, then run: termai sessions branch {}{}",
            self.session.name,
            name.map(|n| format!(" --name {}", n)).unwrap_or_default()
        );

        // Display the branch creation message
        self.say(&self.formatter.format_success(&message));

        // Show branch creation info
        let info_lines = vec![
            "📋 Branch would include:".to_string(),
            format!(
                "   • {} messages from current conversation",
                self.session.messages.len()
            ),
            "   • Full conversation context preserved".to_string(),
            "   • Ready for exploring alternative approaches".to_string(),
        ];

        for line in info_lines {
            println!("  {}", line); // Simple formatting for info lines
        }

        // TODO: Actually create the branch when we have mutable access to repo
        // For now, this demonstrates the UI and command structure
        self.say(&self.formatter.format_warning(
            "⚠️  Branch creation temporarily disabled - requires mutable database access",
        ));

        Ok(())
    }

    /// Handle /theme command
    fn handle_theme_command(&mut self, theme_name: Option<String>) {
        match theme_name {
            Some(name) => match self.formatter.set_theme(&name) {
                Ok(()) => {
                    self.say(
                        &self
                            .formatter
                            .format_success(&format!("Switched to '{}' theme", name)),
                    );
                }
                Err(e) => {
                    self.say(&self.formatter.format_error(&e));
                    let themes = self.formatter.available_themes();
                    self.say(&format!("Available themes: {}", themes.join(", ")));
                }
            },
            None => {
                let themes = self.formatter.available_themes();
                self.say(&format!(
                    "Available themes: {}\nUse '/theme <name>' to switch",
                    themes.join(", ")
                ));
            }
        }
    }

    /// Handle /streaming command
    fn handle_streaming_command(&mut self, setting: Option<bool>) {
        match setting {
            Some(enabled) => {
                self.formatter.set_streaming(enabled);
                let status = if enabled { "enabled" } else { "disabled" };
                self.say(
                    &self
                        .formatter
                        .format_success(&format!("Streaming output {}", status)),
                );
            }
            None => {
                // Toggle: we don't track the current state externally, so just
                // tell the user how to use the command
                self.say(
                    "Usage: /streaming on  - enable streaming output\n       /streaming off - disable streaming output",
                );
            }
        }
    }

    /// Display a settings overview panel
    fn display_settings_overview(&self) {
        let overview = self.formatter.format_settings_overview(
            &self.chat_state.provider,
            &self.chat_state.model,
            self.chat_state.tools_enabled,
            true, // streaming default
            self.context_files.len(),
            &self.session.name,
        );
        self.say(&overview);
    }

    /// Initialize chat state from current configuration
    fn initialize_chat_state(sqlite_repo: &SqliteRepository) -> Result<ChatState> {
        let settings = ResolvedSettings::load_for_current_dir_with_repo(
            sqlite_repo,
            SettingsOverrides::default(),
        )?;
        let chat_state = ChatState::new(
            settings.default_provider.as_str().to_string(),
            settings.selected_model(),
        );

        Ok(chat_state)
    }

    /// Handle model switching command
    async fn handle_model_command(&mut self, model_name: Option<String>) -> Result<()> {
        match model_name {
            Some(model) => {
                // Switch to specified model
                match self.chat_state.switch_model(model) {
                    Ok(message) => {
                        self.say(&self.formatter.format_success(&message));

                        // Update the configuration to reflect the new provider/model
                        self.update_config_from_state().await?;
                    }
                    Err(error) => {
                        self.say(&self.formatter.format_error(&error));
                    }
                }
            }
            None => {
                // Show current model and available models
                self.say(&self.chat_state.status());
                println!();
                self.say(&self.chat_state.list_models());
            }
        }
        Ok(())
    }

    /// Handle provider switching command
    async fn handle_provider_command(&mut self, provider_name: Option<String>) -> Result<()> {
        match provider_name {
            Some(provider) => {
                // Switch to specified provider
                match self.chat_state.switch_provider(provider) {
                    Ok(message) => {
                        self.say(&self.formatter.format_success(&message));

                        // Update the configuration to reflect the new provider/model
                        self.update_config_from_state().await?;
                    }
                    Err(error) => {
                        self.say(&self.formatter.format_error(&error));
                    }
                }
            }
            None => {
                // Show current provider and status
                self.say(&self.chat_state.status());
            }
        }
        Ok(())
    }

    /// Update configuration to reflect current chat state
    async fn update_config_from_state(&self) -> Result<()> {
        let mut user_config = UserConfig::load()?;
        user_config.default.provider = match self.chat_state.provider.as_str() {
            "claude" => SettingsProvider::Claude,
            "openai" => SettingsProvider::Openai,
            "codex" | "openai-codex" | "openai_codex" => SettingsProvider::Codex,
            _ => return Err(anyhow!("Unknown provider: {}", self.chat_state.provider)),
        };
        user_config.default.model = Some(self.chat_state.model.clone());
        user_config.save()?;

        Ok(())
    }
}
