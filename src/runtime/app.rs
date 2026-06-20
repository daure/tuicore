use std::{
    collections::VecDeque,
    fs::{File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use crate::{
    AnimationSettings, EventCtx, EventRoute, FocusKeyBindings, FocusRepair, FocusRequest,
    FocusTarget, HitRegion, HotkeyEvent, HotkeyMatch, HotkeySequenceMatcher, LifecycleCtx,
    Propagation, RuntimeKeyBindings, TreePath, TuiEvent, TuiNode, animation_settings, keybindings,
};

use super::{
    DispatchEffects, EventSource, FocusManager, LayoutEngine, Renderer, Result, Scheduler,
    TerminalGuard, TreeDispatcher,
};

type MessageHandler<N, M> = dyn FnMut(&mut N, M, &mut EventCtx<M>);

pub struct TreeApp<N, M = ()> {
    root: N,
    animation_settings: AnimationSettings,
    runtime_keybindings: RuntimeKeyBindings,
    on_message: Option<Box<MessageHandler<N, M>>>,
}

#[derive(Debug, Default)]
struct RuntimeFlags {
    redraw: bool,
    layout: bool,
    quit: bool,
    focus_request: Option<FocusRequest>,
    focus_repair: Option<FocusRepair>,
    clear: bool,
    wake_animations: bool,
}

pub fn run<N>(root: N) -> Result<()>
where
    N: TuiNode<()>,
{
    TreeApp::new(root).run()
}

impl<N, M> TreeApp<N, M> {
    pub fn new(root: N) -> Self {
        Self {
            root,
            animation_settings: animation_settings(),
            runtime_keybindings: keybindings().runtime().clone(),
            on_message: None,
        }
    }

    pub fn animation_settings(mut self, settings: AnimationSettings) -> Self {
        self.animation_settings = settings;
        self
    }

    pub fn runtime_keybindings(mut self, keybindings: RuntimeKeyBindings) -> Self {
        self.runtime_keybindings = keybindings;
        self
    }

    pub fn on_message(
        mut self,
        handler: impl FnMut(&mut N, M, &mut EventCtx<M>) + 'static,
    ) -> Self {
        self.on_message = Some(Box::new(handler));
        self
    }
}

impl<N, M> TreeApp<N, M>
where
    N: TuiNode<M>,
{
    pub fn run(mut self) -> Result<()> {
        let mut terminal = TerminalGuard::new()?;
        let mut event_source = EventSource::new();
        let mut scheduler = Scheduler::new(self.animation_settings);
        let mut layout_engine = LayoutEngine::new();
        let mut focus_manager = FocusManager::new();
        let mut dispatcher = TreeDispatcher::new();
        let mut renderer = Renderer::new();
        let mut global_hotkeys = HotkeySequenceMatcher::default();
        let mut flags = RuntimeFlags {
            redraw: true,
            layout: true,
            quit: false,
            focus_request: None,
            focus_repair: None,
            clear: false,
            wake_animations: false,
        };

        flags.merge(self.mount_root());

        let run_result = self.run_loop(
            &mut terminal,
            &mut event_source,
            &mut scheduler,
            &mut layout_engine,
            &mut focus_manager,
            &mut dispatcher,
            &mut renderer,
            &mut global_hotkeys,
            &mut flags,
        );

        flags.merge(self.unmount_root());
        let restore_result = terminal.restore();

        if let Err(error) = run_result {
            return Err(error);
        }
        restore_result
    }

    fn run_loop(
        &mut self,
        terminal: &mut TerminalGuard,
        event_source: &mut EventSource,
        scheduler: &mut Scheduler,
        layout_engine: &mut LayoutEngine,
        focus_manager: &mut FocusManager,
        dispatcher: &mut TreeDispatcher,
        renderer: &mut Renderer,
        global_hotkeys: &mut HotkeySequenceMatcher,
        flags: &mut RuntimeFlags,
    ) -> Result<()> {
        let mut global_hotkey_tick = Instant::now();
        while !flags.quit {
            if flags.clear {
                terminal.terminal_mut().clear()?;
                flags.clear = false;
                flags.layout = true;
                flags.redraw = true;
            }

            self.layout_if_pending(flags, focus_manager, layout_engine, dispatcher, terminal)?;

            self.apply_pending_focus(flags, focus_manager, layout_engine, dispatcher);

            self.layout_if_pending(flags, focus_manager, layout_engine, dispatcher, terminal)?;

            if flags.redraw {
                let area = terminal.terminal_mut().size()?.into();
                if flags.layout || layout_engine.area() != area {
                    self.layout_root(flags, focus_manager, layout_engine, dispatcher, area);
                }
                renderer.render(terminal.terminal_mut(), &self.root, area)?;
                flags.redraw = false;
            }

            if flags.wake_animations {
                scheduler.wake();
                flags.wake_animations = false;
            }

            let hotkey_pending_before_poll = global_hotkeys.is_pending();
            if let Some(event) =
                event_source.poll(runtime_poll_timeout(scheduler, global_hotkeys))?
            {
                self.dispatch_runtime_event(
                    Some(terminal),
                    flags,
                    focus_manager,
                    layout_engine,
                    dispatcher,
                    global_hotkeys,
                    event,
                );
            }

            let now = Instant::now();
            let global_hotkey_dt = now.duration_since(global_hotkey_tick);
            global_hotkey_tick = now;
            self.dispatch_global_hotkey_tick_after_poll(
                flags,
                dispatcher,
                layout_engine.focus_targets(),
                global_hotkeys,
                hotkey_pending_before_poll,
                global_hotkey_dt,
            );

            if flags.wake_animations {
                scheduler.wake();
                flags.wake_animations = false;
            }

            if let Some(dt) = scheduler.tick(self.animation_settings.max_dt) {
                let tick = dispatcher.dispatch_tick(&mut self.root, dt, self.animation_settings);
                flags.redraw |= tick.changed || tick.active;
                scheduler.set_active(tick.active);
            }
        }

        Ok(())
    }

    fn mount_root(&mut self) -> RuntimeFlags {
        let mut ctx = LifecycleCtx::default();
        self.root.init(&mut ctx);
        self.root.mount(&mut ctx);
        self.handle_lifecycle(ctx)
    }

    fn unmount_root(&mut self) -> RuntimeFlags {
        let mut ctx = LifecycleCtx::default();
        self.root.unmount(&mut ctx);
        self.root.destroy(&mut ctx);
        self.handle_lifecycle(ctx)
    }

    fn handle_lifecycle(&mut self, mut ctx: LifecycleCtx<M>) -> RuntimeFlags {
        let mut flags = RuntimeFlags {
            redraw: ctx.redraw_requested(),
            layout: ctx.layout_requested(),
            quit: false,
            focus_request: None,
            focus_repair: None,
            clear: false,
            wake_animations: ctx.redraw_requested() || ctx.layout_requested(),
        };
        let messages = ctx.drain_messages().collect();
        self.handle_messages(&mut flags, messages);
        flags
    }

    fn handle_effects(&mut self, effects: DispatchEffects<M>) -> RuntimeFlags {
        let mut flags = RuntimeFlags::from_effects(&effects);
        self.handle_messages(&mut flags, VecDeque::from(effects.messages));
        flags
    }

    fn handle_messages(&mut self, flags: &mut RuntimeFlags, mut messages: VecDeque<M>) {
        while let Some(message) = messages.pop_front() {
            let Some(handler) = self.on_message.as_mut() else {
                continue;
            };

            let mut ctx = EventCtx::new(self.animation_settings);
            handler(&mut self.root, message, &mut ctx);

            flags.redraw |= ctx.redraw_requested();
            flags.layout |= ctx.layout_requested();
            flags.wake_animations |= ctx.redraw_requested() || ctx.layout_requested();
            flags.clear |= ctx.clear_requested();
            flags.quit |= ctx.quit_requested();
            if let Some(request) = ctx.focus_request().cloned() {
                flags.focus_request = Some(request);
            }
            if let Some(repair) = ctx.focus_repair() {
                flags.focus_repair = Some(repair);
            }
            messages.extend(ctx.drain_messages());
        }
    }

    fn layout_if_pending(
        &mut self,
        flags: &mut RuntimeFlags,
        focus_manager: &mut FocusManager,
        layout_engine: &mut LayoutEngine,
        dispatcher: &mut TreeDispatcher,
        terminal: &mut TerminalGuard,
    ) -> Result<()> {
        if flags.layout {
            let area = terminal.terminal_mut().size()?.into();
            self.layout_root(flags, focus_manager, layout_engine, dispatcher, area);
        }
        Ok(())
    }

    fn layout_root(
        &mut self,
        flags: &mut RuntimeFlags,
        focus_manager: &mut FocusManager,
        layout_engine: &mut LayoutEngine,
        dispatcher: &mut TreeDispatcher,
        area: ratatui::layout::Rect,
    ) {
        layout_engine.layout(&mut self.root, area);
        flags.layout = false;
        flags.redraw = true;
        let transition = if flags.focus_request.is_some() {
            flags.focus_repair = None;
            None
        } else if let Some(repair) = flags.focus_repair.take() {
            focus_manager.repair(&repair, layout_engine.focus_targets())
        } else {
            focus_manager.validate(layout_engine.focus_targets())
        };
        if let Some(transition) = transition {
            let effects =
                dispatcher.dispatch_focus(&mut self.root, transition, self.animation_settings);
            flags.merge(self.handle_effects(effects));
        }
    }

    fn apply_pending_focus(
        &mut self,
        flags: &mut RuntimeFlags,
        focus_manager: &mut FocusManager,
        layout_engine: &LayoutEngine,
        dispatcher: &mut TreeDispatcher,
    ) {
        let Some(request) = flags.focus_request.take() else {
            return;
        };
        flags.focus_repair = None;

        if let Some(transition) =
            focus_manager.apply_request(&request, layout_engine.focus_targets())
        {
            let effects =
                dispatcher.dispatch_focus(&mut self.root, transition, self.animation_settings);
            flags.merge(self.handle_effects(effects));
        }
    }

    fn dispatch_hotkey_event_to_targets(
        &mut self,
        flags: &mut RuntimeFlags,
        dispatcher: &mut TreeDispatcher,
        targets: &[FocusTarget],
        hotkey: HotkeyEvent,
    ) {
        let mut seen = Vec::<(TreePath, crate::FocusId)>::new();
        for target in targets.iter().filter(|target| target.enabled) {
            let key = (target.path.clone(), target.id.clone());
            if seen.contains(&key) {
                continue;
            }
            seen.push(key);
            self.dispatch_hotkey_event_to_target(flags, dispatcher, target, hotkey.clone());
        }
    }

    fn dispatch_hotkey_event_to_target(
        &mut self,
        flags: &mut RuntimeFlags,
        dispatcher: &mut TreeDispatcher,
        target: &FocusTarget,
        hotkey: HotkeyEvent,
    ) {
        let route = EventRoute::new(target.path.clone());
        let effects = dispatcher.dispatch_event(
            &mut self.root,
            &route,
            &TuiEvent::Hotkey(hotkey),
            self.animation_settings,
        );
        flags.merge(self.handle_effects(effects));
    }

    fn dispatch_global_hotkey_tick(
        &mut self,
        flags: &mut RuntimeFlags,
        dispatcher: &mut TreeDispatcher,
        targets: &[FocusTarget],
        global_hotkeys: &mut HotkeySequenceMatcher,
        dt: Duration,
    ) {
        if global_hotkeys.tick(dt) {
            self.dispatch_hotkey_event_to_targets(
                flags,
                dispatcher,
                targets,
                HotkeyEvent::Canceled,
            );
            flags.redraw = true;
        }
    }

    fn dispatch_global_hotkey_tick_after_poll(
        &mut self,
        flags: &mut RuntimeFlags,
        dispatcher: &mut TreeDispatcher,
        targets: &[FocusTarget],
        global_hotkeys: &mut HotkeySequenceMatcher,
        was_pending_before_poll: bool,
        dt: Duration,
    ) {
        if was_pending_before_poll {
            self.dispatch_global_hotkey_tick(flags, dispatcher, targets, global_hotkeys, dt);
        }
    }

    fn dispatch_runtime_event(
        &mut self,
        terminal: Option<&mut TerminalGuard>,
        flags: &mut RuntimeFlags,
        focus_manager: &mut FocusManager,
        layout_engine: &LayoutEngine,
        dispatcher: &mut TreeDispatcher,
        global_hotkeys: &mut HotkeySequenceMatcher,
        event: TuiEvent,
    ) {
        let event = event;
        if let TuiEvent::Key(key) = &event {
            if is_global_quit_key(*key, &self.runtime_keybindings) {
                flags.quit = true;
                return;
            }
            let suppress_global_hotkeys = focus_manager
                .current()
                .is_some_and(|target| target.suppress_global_hotkeys);
            if !suppress_global_hotkeys {
                let sequence_targets = hotkey_sequence_targets(layout_engine.focus_targets());
                global_hotkeys.set_hotkeys(sequence_targets.iter().map(|(hotkey, _)| hotkey));
                match global_hotkeys.on_key(*key) {
                    HotkeyMatch::Matched(index) => {
                        self.dispatch_hotkey_event_to_targets(
                            flags,
                            dispatcher,
                            layout_engine.focus_targets(),
                            HotkeyEvent::Canceled,
                        );
                        if let Some((sequence, target)) = sequence_targets.get(index) {
                            self.dispatch_hotkey_event_to_target(
                                flags,
                                dispatcher,
                                target,
                                HotkeyEvent::Commit(sequence.clone()),
                            );
                            if flags.focus_request == Some(FocusRequest::FirstChild) {
                                flags.focus_request = Some(FocusRequest::FirstChildOf {
                                    path: target.path.clone(),
                                    id: target.id.clone(),
                                });
                            }
                            if flags.focus_request.is_none() {
                                flags.focus_request = Some(FocusRequest::TargetAt {
                                    path: target.path.clone(),
                                    id: target.id.clone(),
                                });
                            }
                            if !flags.layout {
                                self.apply_pending_focus(
                                    flags,
                                    focus_manager,
                                    layout_engine,
                                    dispatcher,
                                );
                            }
                        }
                        return;
                    }
                    HotkeyMatch::Pending => {
                        self.dispatch_hotkey_event_to_targets(
                            flags,
                            dispatcher,
                            layout_engine.focus_targets(),
                            HotkeyEvent::Canceled,
                        );
                        let pending_targets =
                            targets_for_prefix(&sequence_targets, global_hotkeys.prefix());
                        self.dispatch_hotkey_event_to_targets(
                            flags,
                            dispatcher,
                            &pending_targets,
                            HotkeyEvent::Pending(global_hotkeys.prefix().to_string()),
                        );
                        flags.redraw = true;
                        return;
                    }
                    HotkeyMatch::Canceled => {
                        self.dispatch_hotkey_event_to_targets(
                            flags,
                            dispatcher,
                            layout_engine.focus_targets(),
                            HotkeyEvent::Canceled,
                        );
                        flags.redraw = true;
                        return;
                    }
                    HotkeyMatch::Ignored => {
                        if global_hotkeys.is_pending() {
                            return;
                        }
                    }
                }
            }
        }

        let route = EventRoute::new(route_path_for_event(
            &event,
            layout_engine.hit_regions(),
            focus_manager.current_path(),
        ));
        if matches!(event, TuiEvent::Resize(_, _)) {
            flags.layout = true;
        }
        let effects =
            dispatcher.dispatch_event(&mut self.root, &route, &event, self.animation_settings);
        let external_editor = effects.external_editor.clone();
        let focus_request = focus_request_from_event(&event, &effects);
        flags.merge(self.handle_effects(effects));
        if flags.focus_request.is_none() {
            flags.focus_request = focus_request;
        }
        if let (Some(terminal), Some(request)) = (terminal, external_editor) {
            self.handle_external_editor(flags, dispatcher, terminal, route, request);
        }
    }

    fn handle_external_editor(
        &mut self,
        flags: &mut RuntimeFlags,
        dispatcher: &mut TreeDispatcher,
        terminal: &mut TerminalGuard,
        route: EventRoute,
        request: crate::ExternalEditorRequest,
    ) {
        flags.clear = true;
        if let Ok(Some(response)) = edit_in_external_editor(terminal, request) {
            let event = TuiEvent::ExternalEditor(response);
            let effects =
                dispatcher.dispatch_event(&mut self.root, &route, &event, self.animation_settings);
            flags.merge(self.handle_effects(effects));
        }
    }

    #[cfg(test)]
    fn run_test_events(
        mut self,
        events: impl IntoIterator<Item = TuiEvent>,
        area: ratatui::layout::Rect,
    ) -> Self {
        let mut scheduler = Scheduler::new(self.animation_settings);
        let mut layout_engine = LayoutEngine::new();
        let mut focus_manager = FocusManager::new();
        let mut dispatcher = TreeDispatcher::new();
        let mut global_hotkeys = HotkeySequenceMatcher::default();
        let mut flags = RuntimeFlags {
            redraw: true,
            layout: true,
            quit: false,
            focus_request: None,
            focus_repair: None,
            clear: false,
            wake_animations: false,
        };

        flags.merge(self.mount_root());
        self.layout_root(
            &mut flags,
            &mut focus_manager,
            &mut layout_engine,
            &mut dispatcher,
            area,
        );

        for event in events {
            if flags.quit {
                break;
            }
            if flags.layout {
                self.layout_root(
                    &mut flags,
                    &mut focus_manager,
                    &mut layout_engine,
                    &mut dispatcher,
                    area,
                );
            }
            self.apply_pending_focus(
                &mut flags,
                &mut focus_manager,
                &layout_engine,
                &mut dispatcher,
            );
            if flags.wake_animations {
                scheduler.wake();
                flags.wake_animations = false;
            }
            self.dispatch_runtime_event(
                None,
                &mut flags,
                &mut focus_manager,
                &layout_engine,
                &mut dispatcher,
                &mut global_hotkeys,
                event,
            );
            if flags.wake_animations {
                scheduler.wake();
                flags.wake_animations = false;
            }
            if let Some(dt) = scheduler.tick(self.animation_settings.max_dt) {
                let tick = dispatcher.dispatch_tick(&mut self.root, dt, self.animation_settings);
                flags.redraw |= tick.changed || tick.active;
                scheduler.set_active(tick.active);
            }
        }

        flags.merge(self.unmount_root());
        self
    }
}

fn route_path_for_event(
    event: &TuiEvent,
    hit_regions: &[HitRegion],
    focused_path: TreePath,
) -> TreePath {
    let TuiEvent::Mouse(mouse) = event else {
        return focused_path;
    };

    hit_regions
        .iter()
        .rev()
        .find(|region| region.contains(mouse.column, mouse.row))
        .map(|region| region.path.clone())
        .unwrap_or(focused_path)
}

fn runtime_poll_timeout(scheduler: &Scheduler, global_hotkeys: &HotkeySequenceMatcher) -> Duration {
    global_hotkeys
        .remaining_timeout()
        .map(|timeout| timeout.min(scheduler.timeout()))
        .unwrap_or_else(|| scheduler.timeout())
}

fn is_global_quit_key(key: crate::KeyEvent, runtime: &RuntimeKeyBindings) -> bool {
    runtime.quit_matches(key)
}

fn edit_in_external_editor(
    terminal: &mut TerminalGuard,
    request: crate::ExternalEditorRequest,
) -> std::io::Result<Option<crate::ExternalEditorResponse>> {
    let temp_files = create_editor_temp_files(&request.value)?;

    let status = terminal.suspend(|| {
        run_editor(
            &temp_files.text_path,
            &temp_files.pos_path,
            request.line,
            request.col,
        )
    });
    let result = match status {
        Ok(status) if status.success() => {
            let content = std::fs::read_to_string(&temp_files.text_path)?;
            let (line, col) = editor_exit_position(&temp_files.pos_path, request.line, request.col);
            Some(crate::ExternalEditorResponse {
                value: content,
                line,
                col,
            })
        }
        _ => None,
    };

    temp_files.cleanup();
    Ok(result)
}

struct EditorTempFiles {
    text_path: PathBuf,
    pos_path: PathBuf,
}

impl EditorTempFiles {
    fn cleanup(self) {
        let _ = std::fs::remove_file(self.text_path);
        let _ = std::fs::remove_file(self.pos_path);
    }
}

fn create_editor_temp_files(value: &str) -> std::io::Result<EditorTempFiles> {
    let (text_path, mut file) = create_unique_temp_file("edit", "txt")?;
    if let Err(error) = file.write_all(value.as_bytes()) {
        let _ = std::fs::remove_file(&text_path);
        return Err(error);
    }
    drop(file);

    let (pos_path, pos_file) = match create_unique_temp_file("edit-pos", "txt") {
        Ok(file) => file,
        Err(error) => {
            let _ = std::fs::remove_file(&text_path);
            return Err(error);
        }
    };
    drop(pos_file);

    Ok(EditorTempFiles {
        text_path,
        pos_path,
    })
}

fn create_unique_temp_file(prefix: &str, extension: &str) -> std::io::Result<(PathBuf, File)> {
    let temp_dir = std::env::temp_dir();
    let process = std::process::id();
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();

    for attempt in 0..1000_u16 {
        let path = temp_dir.join(format!(
            "tuicore-{prefix}-{process}-{seed}-{attempt}.{extension}"
        ));
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => return Ok((path, file)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }

    Err(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "failed to create unique tuicore editor temp file",
    ))
}

fn run_editor(
    temp_path: &Path,
    pos_path: &Path,
    line: usize,
    col: usize,
) -> std::io::Result<std::process::ExitStatus> {
    let editor = std::env::var("EDITOR")
        .or_else(|_| std::env::var("VISUAL"))
        .unwrap_or_else(|_| "vi".to_string());
    let mut cmd = editor_command(&editor, temp_path, pos_path, line, col);
    cmd.status()
}

fn editor_command(
    editor: &str,
    temp_path: &Path,
    pos_path: &Path,
    line: usize,
    col: usize,
) -> Command {
    let mut editor_parts = split_editor_command(editor);
    if editor_parts.is_empty() {
        editor_parts.push("vi".to_string());
    }
    let editor_program = editor_parts.remove(0);
    let editor_bin = Path::new(&editor_program)
        .file_name()
        .map(|file| file.to_string_lossy().to_string())
        .unwrap_or_else(|| editor_program.clone());
    let mut cmd = Command::new(editor_program);
    cmd.args(editor_parts);

    if editor_bin.contains("nano") {
        cmd.arg(format!("+{},{}", line, col));
    } else if editor_bin.contains("emacs") {
        cmd.arg(format!("+{}:{}", line, col));
    } else if is_vi_editor(&editor_bin) {
        cmd.arg(format!("+{}", line));
        cmd.arg("-c");
        cmd.arg(format!(
            "autocmd VimLeavePre * call writefile([string(line('.')), string(col('.'))], '{}')",
            vim_single_quoted_path(pos_path)
        ));
        cmd.arg("-c");
        cmd.arg(format!("normal! {}|", col));
    } else {
        cmd.arg(format!("+{}", line));
    }
    cmd.arg(temp_path);
    cmd
}

fn split_editor_command(editor: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut quote = None;
    let mut escaped = false;

    for ch in editor.chars() {
        if escaped {
            current.push(ch);
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if let Some(quote_char) = quote {
            if ch == quote_char {
                quote = None;
            } else {
                current.push(ch);
            }
            continue;
        }
        if ch == '\'' || ch == '"' {
            quote = Some(ch);
        } else if ch.is_whitespace() {
            if !current.is_empty() {
                parts.push(std::mem::take(&mut current));
            }
        } else {
            current.push(ch);
        }
    }

    if escaped {
        current.push('\\');
    }
    if !current.is_empty() {
        parts.push(current);
    }
    parts
}

fn vim_single_quoted_path(path: &Path) -> String {
    path.to_string_lossy().replace('\'', "''")
}

fn is_vi_editor(editor_bin: &str) -> bool {
    editor_bin.contains("vim") || editor_bin.contains("nvim") || editor_bin.contains("vi")
}

fn editor_exit_position(
    pos_path: &Path,
    default_line: usize,
    default_col: usize,
) -> (usize, usize) {
    let Ok(pos_content) = std::fs::read_to_string(pos_path) else {
        return (default_line, default_col);
    };
    let mut lines = pos_content.lines();
    let line = lines
        .next()
        .and_then(|line| line.parse::<usize>().ok())
        .unwrap_or(default_line);
    let col = lines
        .next()
        .and_then(|col| col.parse::<usize>().ok())
        .unwrap_or(default_col);
    (line, col)
}

fn focus_request_from_event<M>(
    event: &TuiEvent,
    effects: &DispatchEffects<M>,
) -> Option<FocusRequest> {
    let bindings = keybindings();
    focus_request_from_event_with_bindings(event, effects, bindings.focus())
}

fn focus_request_from_event_with_bindings<M>(
    event: &TuiEvent,
    effects: &DispatchEffects<M>,
    focus: &FocusKeyBindings,
) -> Option<FocusRequest> {
    if effects.propagation == Propagation::Stopped || effects.focus_request.is_some() {
        return None;
    }

    let TuiEvent::Key(key) = event else {
        return None;
    };
    if focus.next_matches(*key) {
        Some(FocusRequest::Next)
    } else if focus.previous_matches(*key) {
        Some(FocusRequest::Previous)
    } else if focus.unfocus_matches(*key) {
        Some(FocusRequest::Unfocus)
    } else {
        None
    }
}

impl RuntimeFlags {
    fn from_effects<M>(effects: &DispatchEffects<M>) -> Self {
        Self {
            redraw: effects.redraw,
            layout: effects.layout,
            quit: effects.quit,
            focus_request: effects.focus_request.clone(),
            focus_repair: effects.focus_repair,
            clear: effects.clear,
            wake_animations: effects.redraw || effects.layout,
        }
    }

    fn merge(&mut self, other: Self) {
        self.redraw |= other.redraw;
        self.layout |= other.layout;
        self.quit |= other.quit;
        self.clear |= other.clear;
        self.wake_animations |= other.wake_animations;
        if other.focus_request.is_some() {
            self.focus_request = other.focus_request;
        }
        if other.focus_repair.is_some() {
            self.focus_repair = other.focus_repair;
        }
    }
}

fn hotkey_sequence_targets(targets: &[FocusTarget]) -> Vec<(String, FocusTarget)> {
    targets
        .iter()
        .filter(|target| target.enabled)
        .flat_map(|target| {
            target
                .hotkey_sequences
                .iter()
                .cloned()
                .map(|hotkey| (hotkey, target.clone()))
        })
        .collect()
}

fn targets_for_prefix(targets: &[(String, FocusTarget)], prefix: &str) -> Vec<FocusTarget> {
    let mut found = Vec::new();
    for (_, target) in targets
        .iter()
        .filter(|(hotkey, _)| hotkey.starts_with(prefix))
    {
        if !found.iter().any(|other| same_focus_target(other, target)) {
            found.push(target.clone());
        }
    }
    found
}

fn same_focus_target(a: &FocusTarget, b: &FocusTarget) -> bool {
    a.id == b.id && a.path == b.path
}

#[cfg(test)]
mod tests {
    use ratatui::{Frame, layout::Rect};

    use super::*;
    use crate::{
        ChildKey, EventOutcome, Flex, FlexItem, FocusCtx, FocusId, FocusTarget, Key, KeyEvent,
        KeyModifiers, KeySpec, LayoutCtx, LayoutResult, MouseButton, MouseEvent, MouseEventKind,
        Preset, RuntimeKeyBindings, TreePath, preset, set_preset,
    };

    #[derive(Default)]
    struct QuitNode {
        mounted: bool,
        destroyed: bool,
        events: usize,
    }

    #[derive(Default)]
    struct LifecycleMessageNode {
        messages: Vec<&'static str>,
    }

    #[derive(Default)]
    struct DynamicFocusNode {
        show_new: bool,
        events: usize,
        focused: Option<String>,
    }

    struct RemoveFocusedMiddleNode {
        flex: Flex,
        focus_log: std::rc::Rc<std::cell::RefCell<Vec<(String, bool)>>>,
        removed: bool,
    }

    #[derive(Default)]
    struct ModalRestoreNode {
        active: bool,
        focused: Vec<String>,
    }

    struct FocusProbe {
        name: &'static str,
        focus_log: std::rc::Rc<std::cell::RefCell<Vec<(String, bool)>>>,
    }

    struct MouseProbe {
        name: &'static str,
        event_log: std::rc::Rc<std::cell::RefCell<Vec<String>>>,
    }

    struct MouseRouteNode {
        flex: Flex,
        event_log: std::rc::Rc<std::cell::RefCell<Vec<String>>>,
    }

    struct HotkeyRouteNode {
        flex: Flex,
        event_log: std::rc::Rc<std::cell::RefCell<Vec<String>>>,
    }

    #[derive(Default)]
    struct HotkeyLayoutFocusNode {
        show_new: bool,
        focused: Option<String>,
    }

    struct HotkeyProbe {
        name: &'static str,
        hotkey: &'static str,
        event_log: std::rc::Rc<std::cell::RefCell<Vec<String>>>,
    }

    #[derive(Default)]
    struct SuppressedHotkeyNode {
        key_events: usize,
        hotkey_events: usize,
    }

    impl TuiNode<()> for QuitNode {
        fn layout(&mut self, area: Rect, _ctx: &mut LayoutCtx) -> LayoutResult {
            LayoutResult::new(area)
        }

        fn render(&self, _frame: &mut Frame, _area: Rect) {}

        fn event(&mut self, _event: &TuiEvent, ctx: &mut EventCtx<()>) -> EventOutcome {
            self.events += 1;
            ctx.request_quit();
            EventOutcome::Handled
        }

        fn mount(&mut self, _ctx: &mut LifecycleCtx<()>) {
            self.mounted = true;
        }

        fn destroy(&mut self, _ctx: &mut LifecycleCtx<()>) {
            self.destroyed = true;
        }
    }

    impl TuiNode<&'static str> for LifecycleMessageNode {
        fn layout(&mut self, area: Rect, _ctx: &mut LayoutCtx) -> LayoutResult {
            LayoutResult::new(area)
        }

        fn render(&self, _frame: &mut Frame, _area: Rect) {}

        fn mount(&mut self, ctx: &mut LifecycleCtx<&'static str>) {
            ctx.emit("mounted");
        }

        fn unmount(&mut self, ctx: &mut LifecycleCtx<&'static str>) {
            ctx.emit("unmounted");
        }
    }

    impl TuiNode<()> for DynamicFocusNode {
        fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
            ctx.register_focusable(FocusId::new("old"), area, true);
            if self.show_new {
                ctx.register_focusable(FocusId::new("new"), area, true);
            }
            LayoutResult::new(area)
        }

        fn render(&self, _frame: &mut Frame, _area: Rect) {}

        fn event(&mut self, _event: &TuiEvent, ctx: &mut EventCtx<()>) -> EventOutcome {
            self.events += 1;
            if self.events == 1 {
                self.show_new = true;
                ctx.request_layout();
                ctx.focus(FocusRequest::TargetAt {
                    path: TreePath::new(),
                    id: FocusId::new("new"),
                });
            } else {
                ctx.request_quit();
            }
            EventOutcome::Handled
        }

        fn focus(&mut self, target: Option<&FocusId>, focused: bool, _ctx: &mut FocusCtx<()>) {
            if focused {
                self.focused = target.map(|target| target.as_str().to_owned());
            }
        }
    }

    impl RemoveFocusedMiddleNode {
        fn new() -> Self {
            let focus_log = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
            let flex = Flex::row()
                .child(
                    "one",
                    FocusProbe {
                        name: "one",
                        focus_log: std::rc::Rc::clone(&focus_log),
                    },
                    FlexItem::fixed(1),
                )
                .child(
                    "two",
                    FocusProbe {
                        name: "two",
                        focus_log: std::rc::Rc::clone(&focus_log),
                    },
                    FlexItem::fixed(1),
                )
                .child(
                    "three",
                    FocusProbe {
                        name: "three",
                        focus_log: std::rc::Rc::clone(&focus_log),
                    },
                    FlexItem::fixed(1),
                );

            Self {
                flex,
                focus_log,
                removed: false,
            }
        }
    }

    impl TuiNode<()> for RemoveFocusedMiddleNode {
        fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
            self.flex.layout(area, ctx)
        }

        fn render(&self, frame: &mut Frame, area: Rect) {
            self.flex.render(frame, area);
        }

        fn dispatch_event(
            &mut self,
            route: &EventRoute,
            event: &TuiEvent,
            ctx: &mut EventCtx<()>,
        ) -> EventOutcome {
            if route.path.first() == Some(&ChildKey::from("two")) && !self.removed {
                self.removed = true;
                self.flex.remove("two", ctx).unwrap();
                EventOutcome::Handled
            } else {
                self.flex.dispatch_event(route, event, ctx)
            }
        }

        fn dispatch_focus(&mut self, target: &FocusTarget, focused: bool, ctx: &mut FocusCtx<()>) {
            self.flex.dispatch_focus(target, focused, ctx);
        }
    }

    impl TuiNode<()> for FocusProbe {
        fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
            ctx.register_focusable(FocusId::new(self.name), area, true);
            LayoutResult::new(area)
        }

        fn render(&self, _frame: &mut Frame, _area: Rect) {}

        fn event(&mut self, _event: &TuiEvent, _ctx: &mut EventCtx<()>) -> EventOutcome {
            EventOutcome::Ignored
        }

        fn focus(&mut self, _target: Option<&FocusId>, focused: bool, _ctx: &mut FocusCtx<()>) {
            self.focus_log
                .borrow_mut()
                .push((self.name.to_owned(), focused));
        }
    }

    impl TuiNode<()> for ModalRestoreNode {
        fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
            if self.active {
                ctx.register_focusable(FocusId::new("dialog"), area, true);
            } else {
                let left = Rect::new(area.x, area.y, 1, area.height);
                let right = Rect::new(area.x.saturating_add(1), area.y, 1, area.height);
                ctx.register_focusable(FocusId::new("one"), left, true);
                ctx.register_focusable(FocusId::new("two"), right, true);
            }
            LayoutResult::new(area)
        }

        fn render(&self, _frame: &mut Frame, _area: Rect) {}

        fn dispatch_event(
            &mut self,
            _route: &EventRoute,
            event: &TuiEvent,
            ctx: &mut EventCtx<()>,
        ) -> EventOutcome {
            let TuiEvent::Key(key) = event else {
                return EventOutcome::Ignored;
            };
            if !self.active && key.code == Key::Enter {
                self.active = true;
                ctx.request_layout();
                ctx.stop_propagation();
                return EventOutcome::Handled;
            }
            if self.active && key.code == Key::Char('x') {
                self.active = false;
                ctx.focus(FocusRequest::Last);
                ctx.request_layout();
                ctx.stop_propagation();
                return EventOutcome::Handled;
            }
            EventOutcome::Ignored
        }

        fn focus(&mut self, target: Option<&FocusId>, focused: bool, _ctx: &mut FocusCtx<()>) {
            if focused {
                if let Some(target) = target {
                    self.focused.push(target.as_str().to_owned());
                }
            }
        }
    }

    impl MouseRouteNode {
        fn new() -> Self {
            let event_log = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
            let flex = Flex::row()
                .child(
                    "left",
                    MouseProbe {
                        name: "left",
                        event_log: std::rc::Rc::clone(&event_log),
                    },
                    FlexItem::fixed(5),
                )
                .child(
                    "right",
                    MouseProbe {
                        name: "right",
                        event_log: std::rc::Rc::clone(&event_log),
                    },
                    FlexItem::fixed(5),
                );

            Self { flex, event_log }
        }
    }

    impl TuiNode<()> for MouseRouteNode {
        fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
            self.flex.layout(area, ctx)
        }

        fn render(&self, frame: &mut Frame, area: Rect) {
            self.flex.render(frame, area);
        }

        fn dispatch_event(
            &mut self,
            route: &EventRoute,
            event: &TuiEvent,
            ctx: &mut EventCtx<()>,
        ) -> EventOutcome {
            self.flex.dispatch_event(route, event, ctx)
        }
    }

    impl TuiNode<()> for MouseProbe {
        fn layout(&mut self, area: Rect, _ctx: &mut LayoutCtx) -> LayoutResult {
            LayoutResult::new(area)
        }

        fn render(&self, _frame: &mut Frame, _area: Rect) {}

        fn event(&mut self, event: &TuiEvent, ctx: &mut EventCtx<()>) -> EventOutcome {
            if matches!(event, TuiEvent::Mouse(_)) {
                self.event_log.borrow_mut().push(self.name.to_owned());
                ctx.request_quit();
                EventOutcome::Handled
            } else {
                EventOutcome::Ignored
            }
        }
    }

    impl HotkeyRouteNode {
        fn new() -> Self {
            let event_log = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
            let flex = Flex::row()
                .child(
                    "save",
                    HotkeyProbe {
                        name: "save",
                        hotkey: "sa",
                        event_log: std::rc::Rc::clone(&event_log),
                    },
                    FlexItem::fixed(5),
                )
                .child(
                    "settings",
                    HotkeyProbe {
                        name: "settings",
                        hotkey: "st",
                        event_log: std::rc::Rc::clone(&event_log),
                    },
                    FlexItem::fixed(5),
                );

            Self { flex, event_log }
        }
    }

    impl TuiNode<()> for HotkeyRouteNode {
        fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
            self.flex.layout(area, ctx)
        }

        fn render(&self, frame: &mut Frame, area: Rect) {
            self.flex.render(frame, area);
        }

        fn dispatch_event(
            &mut self,
            route: &EventRoute,
            event: &TuiEvent,
            ctx: &mut EventCtx<()>,
        ) -> EventOutcome {
            self.flex.dispatch_event(route, event, ctx)
        }
    }

    impl TuiNode<()> for HotkeyProbe {
        fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
            ctx.register_focusable_with_hotkey_sequences(
                FocusId::new(self.name),
                area,
                true,
                vec![self.hotkey.to_string()],
            );
            LayoutResult::new(area)
        }

        fn render(&self, _frame: &mut Frame, _area: Rect) {}

        fn event(&mut self, event: &TuiEvent, _ctx: &mut EventCtx<()>) -> EventOutcome {
            if let TuiEvent::Hotkey(hotkey) = event {
                self.event_log
                    .borrow_mut()
                    .push(format!("{}:{hotkey:?}", self.name));
            }
            EventOutcome::Ignored
        }
    }

    impl TuiNode<()> for SuppressedHotkeyNode {
        fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
            ctx.register_focusable(FocusId::new("text"), area, true);
            ctx.set_focus_suppresses_global_hotkeys(FocusId::new("text"), true);
            ctx.register_focusable_with_hotkey_sequences(
                FocusId::new("action"),
                area,
                true,
                vec!["sa".to_string()],
            );
            LayoutResult::new(area)
        }

        fn render(&self, _frame: &mut Frame, _area: Rect) {}

        fn event(&mut self, event: &TuiEvent, _ctx: &mut EventCtx<()>) -> EventOutcome {
            match event {
                TuiEvent::Key(_) => self.key_events += 1,
                TuiEvent::Hotkey(_) => self.hotkey_events += 1,
                _ => {}
            }
            EventOutcome::Ignored
        }
    }

    impl TuiNode<()> for HotkeyLayoutFocusNode {
        fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
            ctx.register_focusable_with_hotkey_sequences(
                FocusId::new("trigger"),
                area,
                true,
                vec!["sa".to_string()],
            );
            if self.show_new {
                ctx.register_focusable(FocusId::new("new"), area, true);
            }
            LayoutResult::new(area)
        }

        fn render(&self, _frame: &mut Frame, _area: Rect) {}

        fn event(&mut self, event: &TuiEvent, ctx: &mut EventCtx<()>) -> EventOutcome {
            if matches!(event, TuiEvent::Hotkey(HotkeyEvent::Commit(sequence)) if sequence == "sa")
            {
                self.show_new = true;
                ctx.request_layout();
                ctx.focus(FocusRequest::TargetAt {
                    path: TreePath::new(),
                    id: FocusId::new("new"),
                });
                return EventOutcome::Handled;
            }
            EventOutcome::Ignored
        }

        fn focus(&mut self, target: Option<&FocusId>, focused: bool, _ctx: &mut FocusCtx<()>) {
            if focused {
                self.focused = target.map(|target| target.as_str().to_owned());
            }
        }
    }

    fn effects(propagation: Propagation) -> DispatchEffects<()> {
        DispatchEffects {
            outcome: EventOutcome::Ignored,
            messages: Vec::new(),
            redraw: false,
            layout: false,
            quit: false,
            focus_request: None,
            focus_repair: None,
            propagation,
            clear: false,
            external_editor: None,
        }
    }

    #[test]
    fn test_loop_seam_mounts_dispatches_quit_and_unmounts() {
        let app = TreeApp::new(QuitNode::default());
        let event = TuiEvent::Key(KeyEvent::from(Key::Enter));

        let app = app.run_test_events([event], Rect::new(0, 0, 20, 5));

        assert!(app.root.mounted);
        assert!(app.root.destroyed);
        assert_eq!(app.root.events, 1);
    }

    #[test]
    fn lifecycle_messages_are_delivered_on_mount_and_unmount() {
        let app =
            TreeApp::new(LifecycleMessageNode::default()).on_message(|root, message, _ctx| {
                root.messages.push(message);
            });
        let event = TuiEvent::Key(KeyEvent::from(Key::Enter));

        let app = app.run_test_events([event], Rect::new(0, 0, 20, 5));

        assert_eq!(app.root.messages, vec!["mounted", "unmounted"]);
    }

    #[test]
    fn lifecycle_message_clear_request_reaches_runtime_flags() {
        let mut app =
            TreeApp::new(LifecycleMessageNode::default()).on_message(|root, message, ctx| {
                root.messages.push(message);
                ctx.request_clear();
            });

        let flags = app.mount_root();

        assert!(flags.clear);
        assert!(flags.redraw);
    }

    #[test]
    fn pending_layout_runs_before_pending_focus_request() {
        let app = TreeApp::new(DynamicFocusNode::default());
        let event = TuiEvent::Key(KeyEvent::from(Key::Enter));

        let app = app.run_test_events([event.clone(), event], Rect::new(0, 0, 20, 5));

        assert_eq!(app.root.focused.as_deref(), Some("new"));
    }

    #[test]
    fn explicit_focus_request_clears_pending_focus_repair() {
        let mut app = TreeApp::new(QuitNode::default());
        let mut flags = RuntimeFlags {
            redraw: false,
            layout: false,
            quit: false,
            focus_request: Some(FocusRequest::Unfocus),
            focus_repair: Some(FocusRepair::RemovedChild { index: 0 }),
            clear: false,
            wake_animations: false,
        };
        let mut focus_manager = FocusManager::new();
        let layout_engine = LayoutEngine::new();
        let mut dispatcher = TreeDispatcher::new();

        app.apply_pending_focus(
            &mut flags,
            &mut focus_manager,
            &layout_engine,
            &mut dispatcher,
        );

        assert_eq!(flags.focus_repair, None);
    }

    #[test]
    fn pending_last_focus_request_restores_focus_after_modal_layout() {
        let app = TreeApp::new(ModalRestoreNode::default());
        let events = [
            TuiEvent::Key(KeyEvent::from(Key::Tab)),
            TuiEvent::Key(KeyEvent::from(Key::Enter)),
            TuiEvent::Key(KeyEvent::from(Key::Char('x'))),
            TuiEvent::Key(KeyEvent::from(Key::Null)),
        ];

        let app = app.run_test_events(events, Rect::new(0, 0, 20, 5));

        assert_eq!(app.root.focused.last().map(String::as_str), Some("two"));
    }

    #[test]
    fn ctrl_q_quits_even_when_modal_handles_other_keys() {
        let app = TreeApp::new(ModalRestoreNode::default());
        let events = [
            TuiEvent::Key(KeyEvent::from(Key::Enter)),
            TuiEvent::Key(KeyEvent {
                code: Key::Char('q'),
                modifiers: KeyModifiers::CONTROL,
            }),
            TuiEvent::Key(KeyEvent::from(Key::Char('x'))),
        ];

        let app = app.run_test_events(events, Rect::new(0, 0, 20, 5));

        assert!(app.root.active);
    }

    #[test]
    fn runtime_quit_keybinding_can_be_remapped() {
        let app = TreeApp::new(ModalRestoreNode::default())
            .runtime_keybindings(RuntimeKeyBindings::new().with_quit([KeySpec::plain('x')]));
        let events = [
            TuiEvent::Key(KeyEvent::from(Key::Enter)),
            TuiEvent::Key(KeyEvent {
                code: Key::Char('q'),
                modifiers: KeyModifiers::CONTROL,
            }),
            TuiEvent::Key(KeyEvent::from(Key::Char('x'))),
        ];

        let app = app.run_test_events(events, Rect::new(0, 0, 20, 5));

        assert!(app.root.active);
    }

    #[test]
    fn removing_focused_middle_child_repairs_focus_to_next_child() {
        let app = TreeApp::new(RemoveFocusedMiddleNode::new());
        let tab = TuiEvent::Key(KeyEvent::from(Key::Tab));
        let enter = TuiEvent::Key(KeyEvent::from(Key::Enter));

        let app = app.run_test_events([tab, enter.clone(), enter], Rect::new(0, 0, 3, 1));

        assert!(app.root.removed);
        assert!(
            app.root
                .focus_log
                .borrow()
                .contains(&("three".to_owned(), true))
        );
    }

    #[test]
    fn new_uses_global_animation_settings_by_default() {
        let _guard = PresetGuard::replace(Preset::new().with_animation(AnimationSettings {
            enabled: false,
            ..AnimationSettings::default()
        }));

        let app = TreeApp::<_, ()>::new(QuitNode::default());

        assert!(!app.animation_settings.enabled);
    }

    #[test]
    fn default_focus_keybindings_enqueue_next_and_previous_requests() {
        let next = TuiEvent::Key(KeyEvent::from(Key::Tab));
        let previous = TuiEvent::Key(KeyEvent::from(Key::BackTab));
        let esc_unfocus = TuiEvent::Key(KeyEvent::from(Key::Esc));
        let ctrl_left_bracket_unfocus = TuiEvent::Key(KeyEvent {
            code: Key::Char('['),
            modifiers: KeyModifiers::CONTROL,
        });
        let bindings = FocusKeyBindings::default();

        assert_eq!(
            focus_request_from_event_with_bindings(
                &next,
                &effects(Propagation::Continue),
                &bindings
            ),
            Some(FocusRequest::Next)
        );
        assert_eq!(
            focus_request_from_event_with_bindings(
                &previous,
                &effects(Propagation::Continue),
                &bindings
            ),
            Some(FocusRequest::Previous)
        );
        assert_eq!(
            focus_request_from_event_with_bindings(
                &esc_unfocus,
                &effects(Propagation::Continue),
                &bindings
            ),
            Some(FocusRequest::Unfocus)
        );
        assert_eq!(
            focus_request_from_event_with_bindings(
                &ctrl_left_bracket_unfocus,
                &effects(Propagation::Continue),
                &bindings
            ),
            Some(FocusRequest::Unfocus)
        );
    }

    #[test]
    fn custom_focus_keybindings_enqueue_configured_requests() {
        let bindings = FocusKeyBindings::new()
            .with_next([KeySpec::plain('l')])
            .with_previous([KeySpec::plain('h')]);
        let next = TuiEvent::Key(KeyEvent {
            code: Key::Char('l'),
            modifiers: KeyModifiers::NONE,
        });
        let previous = TuiEvent::Key(KeyEvent {
            code: Key::Char('h'),
            modifiers: KeyModifiers::NONE,
        });

        assert_eq!(
            focus_request_from_event_with_bindings(
                &next,
                &effects(Propagation::Continue),
                &bindings
            ),
            Some(FocusRequest::Next)
        );
        assert_eq!(
            focus_request_from_event_with_bindings(
                &previous,
                &effects(Propagation::Continue),
                &bindings
            ),
            Some(FocusRequest::Previous)
        );
    }

    #[test]
    fn focus_keybindings_do_not_run_after_stopped_propagation() {
        let event = TuiEvent::Key(KeyEvent::from(Key::Tab));
        let bindings = FocusKeyBindings::default();

        assert_eq!(
            focus_request_from_event_with_bindings(
                &event,
                &effects(Propagation::Stopped),
                &bindings
            ),
            None
        );
    }

    #[test]
    fn external_editor_command_uses_program_arguments_without_shell() {
        let temp_path = Path::new("/tmp/tuicore edit.txt");
        let pos_path = Path::new("/tmp/tuicore pos.txt");

        let command = editor_command("vim -u NONE", temp_path, pos_path, 3, 4);

        assert_eq!(command.get_program(), "vim");
        assert!(command.get_args().any(|arg| arg == "-u"));
        assert!(command.get_args().any(|arg| arg == "NONE"));
        assert!(command.get_args().any(|arg| arg == temp_path.as_os_str()));
    }

    #[test]
    fn editor_temp_files_are_unique_and_created_with_contents() {
        let first = create_editor_temp_files("one").expect("first temp files should be created");
        let second = create_editor_temp_files("two").expect("second temp files should be created");

        assert_ne!(first.text_path, second.text_path);
        assert_ne!(first.pos_path, second.pos_path);
        assert_eq!(std::fs::read_to_string(&first.text_path).unwrap(), "one");
        assert_eq!(std::fs::read_to_string(&second.text_path).unwrap(), "two");

        first.cleanup();
        second.cleanup();
    }

    #[test]
    fn custom_focus_keybinding_does_not_run_after_child_consumes_key() {
        let event = TuiEvent::Key(KeyEvent::from(Key::Char('x')));
        let bindings = FocusKeyBindings::new().with_next([KeySpec::plain('x')]);
        let mut effects = effects(Propagation::Continue);
        effects.outcome = EventOutcome::Handled;
        effects.propagation = Propagation::Stopped;

        assert_eq!(
            focus_request_from_event_with_bindings(&event, &effects, &bindings),
            None
        );
    }

    #[test]
    fn mouse_events_route_to_topmost_hit_region() {
        let event = TuiEvent::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 3,
            row: 1,
            modifiers: KeyModifiers::NONE,
        });
        let hit_regions = [
            HitRegion::new(
                TreePath::from_keys([ChildKey::from("back")]),
                Rect::new(0, 0, 5, 5),
            ),
            HitRegion::new(
                TreePath::from_keys([ChildKey::from("front")]),
                Rect::new(0, 0, 5, 5),
            ),
        ];

        let path = route_path_for_event(&event, &hit_regions, TreePath::new());

        assert_eq!(path.keys(), &[ChildKey::from("front")]);
    }

    #[test]
    fn mouse_events_dispatch_to_hit_flex_child() {
        let app = TreeApp::new(MouseRouteNode::new());
        let event = TuiEvent::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 7,
            row: 0,
            modifiers: KeyModifiers::NONE,
        });

        let app = app.run_test_events([event], Rect::new(0, 0, 10, 1));

        assert_eq!(app.root.event_log.borrow().as_slice(), ["right"]);
    }

    #[test]
    fn pending_global_hotkey_finds_all_matching_targets() {
        let current = FocusTarget {
            id: FocusId::new("tabs"),
            path: TreePath::from_keys([ChildKey::new("first")]),
            area: Rect::default(),
            enabled: true,
            tab_stop: true,
            hotkey: None,
            hotkeys: Vec::new(),
            hotkey_sequences: vec!["s".to_string()],
            suppress_global_hotkeys: false,
        };
        let other = FocusTarget {
            id: FocusId::new("tabs"),
            path: TreePath::from_keys([ChildKey::new("second")]),
            area: Rect::default(),
            enabled: true,
            tab_stop: true,
            hotkey: None,
            hotkeys: Vec::new(),
            hotkey_sequences: vec!["sa".to_string()],
            suppress_global_hotkeys: false,
        };
        let targets = hotkey_sequence_targets(&[current.clone(), other]);

        let targets = targets_for_prefix(&targets, "s");

        assert_eq!(targets.len(), 2);
        assert_eq!(targets[0].path, current.path);
    }

    #[test]
    fn pending_global_hotkey_includes_exact_and_longer_targets() {
        let exact = FocusTarget {
            id: FocusId::new("toggle"),
            path: TreePath::from_keys([ChildKey::new("theme")]),
            area: Rect::default(),
            enabled: true,
            tab_stop: true,
            hotkey: None,
            hotkeys: Vec::new(),
            hotkey_sequences: vec!["t".to_string()],
            suppress_global_hotkeys: false,
        };
        let longer = FocusTarget {
            id: FocusId::new("tabs"),
            path: TreePath::from_keys([ChildKey::new("tabs")]),
            area: Rect::default(),
            enabled: true,
            tab_stop: true,
            hotkey: None,
            hotkeys: Vec::new(),
            hotkey_sequences: vec!["ta".to_string()],
            suppress_global_hotkeys: false,
        };
        let targets = hotkey_sequence_targets(&[exact, longer.clone()]);

        let targets = targets_for_prefix(&targets, "t");

        assert_eq!(targets.len(), 2);
        assert!(targets.iter().any(|target| target.path == longer.path));
    }

    #[test]
    fn focused_text_target_suppresses_global_hotkey_sequences() {
        let app = TreeApp::new(SuppressedHotkeyNode::default());
        let events = [
            TuiEvent::Key(KeyEvent::from(Key::Char('s'))),
            TuiEvent::Key(KeyEvent::from(Key::Char('a'))),
        ];

        let app = app.run_test_events(events, Rect::new(0, 0, 10, 1));

        assert_eq!(app.root.key_events, 2);
        assert_eq!(app.root.hotkey_events, 0);
    }

    #[test]
    fn pending_global_hotkey_times_out_when_animations_are_disabled() {
        let mut animation = AnimationSettings::default();
        animation.enabled = false;
        let mut app = TreeApp::new(HotkeyRouteNode::new()).animation_settings(animation);
        let mut scheduler = Scheduler::new(animation);
        let mut layout_engine = LayoutEngine::new();
        let mut focus_manager = FocusManager::new();
        let mut dispatcher = TreeDispatcher::new();
        let mut global_hotkeys = HotkeySequenceMatcher::default();
        let mut flags = RuntimeFlags {
            redraw: true,
            layout: true,
            quit: false,
            focus_request: None,
            focus_repair: None,
            clear: false,
            wake_animations: false,
        };

        flags.merge(app.mount_root());
        app.layout_root(
            &mut flags,
            &mut focus_manager,
            &mut layout_engine,
            &mut dispatcher,
            Rect::new(0, 0, 10, 1),
        );
        app.dispatch_runtime_event(
            None,
            &mut flags,
            &mut focus_manager,
            &layout_engine,
            &mut dispatcher,
            &mut global_hotkeys,
            TuiEvent::Key(KeyEvent::from(Key::Char('s'))),
        );

        assert!(runtime_poll_timeout(&scheduler, &global_hotkeys) <= crate::hotkey::HOTKEY_TIMEOUT);
        app.dispatch_global_hotkey_tick(
            &mut flags,
            &mut dispatcher,
            layout_engine.focus_targets(),
            &mut global_hotkeys,
            crate::hotkey::HOTKEY_TIMEOUT,
        );

        let events = app.root.event_log.borrow();
        assert!(events.iter().any(|event| event == "save:Pending(\"s\")"));
        assert!(events.iter().any(|event| event == "save:Canceled"));
        assert!(!global_hotkeys.is_pending());
        assert_eq!(scheduler.tick(animation.max_dt), None);
    }

    #[test]
    fn disabled_animation_idle_before_first_pending_key_does_not_cancel_hotkey() {
        let mut animation = AnimationSettings::default();
        animation.enabled = false;
        let mut app = TreeApp::new(HotkeyRouteNode::new()).animation_settings(animation);
        let mut scheduler = Scheduler::new(animation);
        let mut layout_engine = LayoutEngine::new();
        let mut focus_manager = FocusManager::new();
        let mut dispatcher = TreeDispatcher::new();
        let mut global_hotkeys = HotkeySequenceMatcher::default();
        let mut flags = RuntimeFlags {
            redraw: true,
            layout: true,
            quit: false,
            focus_request: None,
            focus_repair: None,
            clear: false,
            wake_animations: false,
        };

        flags.merge(app.mount_root());
        app.layout_root(
            &mut flags,
            &mut focus_manager,
            &mut layout_engine,
            &mut dispatcher,
            Rect::new(0, 0, 10, 1),
        );
        app.dispatch_runtime_event(
            None,
            &mut flags,
            &mut focus_manager,
            &layout_engine,
            &mut dispatcher,
            &mut global_hotkeys,
            TuiEvent::Key(KeyEvent::from(Key::Char('s'))),
        );
        let events_before_idle_tick = app.root.event_log.borrow().len();

        app.dispatch_global_hotkey_tick_after_poll(
            &mut flags,
            &mut dispatcher,
            layout_engine.focus_targets(),
            &mut global_hotkeys,
            false,
            crate::hotkey::HOTKEY_TIMEOUT,
        );

        assert!(global_hotkeys.is_pending());
        assert_eq!(global_hotkeys.prefix(), "s");
        assert_eq!(scheduler.tick(animation.max_dt), None);
        assert_eq!(app.root.event_log.borrow().len(), events_before_idle_tick);
    }

    #[test]
    fn hotkey_commit_focus_request_waits_for_requested_layout() {
        let app = TreeApp::new(HotkeyLayoutFocusNode::default());
        let events = [
            TuiEvent::Key(KeyEvent::from(Key::Char('s'))),
            TuiEvent::Key(KeyEvent::from(Key::Char('a'))),
            TuiEvent::Key(KeyEvent::from(Key::Null)),
        ];

        let app = app.run_test_events(events, Rect::new(0, 0, 20, 5));

        assert_eq!(app.root.focused.as_deref(), Some("new"));
    }

    #[test]
    fn diverging_hotkey_prefix_cancels_previous_pending_targets() {
        let app = TreeApp::new(HotkeyRouteNode::new());
        let events = [
            TuiEvent::Key(KeyEvent::from(Key::Char('s'))),
            TuiEvent::Key(KeyEvent::from(Key::Char('a'))),
        ];

        let app = app.run_test_events(events, Rect::new(0, 0, 10, 1));
        let events = app.root.event_log.borrow();

        assert!(
            events
                .iter()
                .any(|event| event == "settings:Pending(\"s\")")
        );
        assert!(events.iter().any(|event| event == "settings:Canceled"));
        assert!(events.iter().any(|event| event == "save:Commit(\"sa\")"));
    }

    struct PresetGuard {
        previous: Preset,
        _lock: std::sync::MutexGuard<'static, ()>,
    }

    impl PresetGuard {
        fn replace(next: Preset) -> Self {
            let lock = crate::ENV_LOCK.lock().expect("test env lock should lock");
            let previous = preset();
            set_preset(next);
            Self {
                previous,
                _lock: lock,
            }
        }
    }

    impl Drop for PresetGuard {
        fn drop(&mut self) {
            set_preset(self.previous.clone());
        }
    }
}
