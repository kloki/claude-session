use std::{
    io::Write,
    sync::mpsc,
    time::{Duration, Instant},
};

use crossterm::{
    cursor,
    event::{Event, KeyCode, KeyModifiers},
    execute,
    terminal::{self, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};
use notify::{RecursiveMode, Watcher};

use crate::{
    output,
    session::{self, SessionStore},
};

struct CleanupGuard;

impl Drop for CleanupGuard {
    fn drop(&mut self) {
        let _ = terminal::disable_raw_mode();
        let _ = execute!(std::io::stdout(), LeaveAlternateScreen, cursor::Show);
    }
}

fn is_session_file_event(event: &notify::Event) -> bool {
    event
        .paths
        .iter()
        .any(|p| p.file_name() == Some(".claude_sessions".as_ref()))
}

fn render() -> anyhow::Result<()> {
    let store = SessionStore::load_and_cleanup()?;
    let table = output::format_ps(&store, true, None);

    let mut stdout = std::io::stdout();
    execute!(
        stdout,
        cursor::MoveTo(0, 0),
        terminal::Clear(ClearType::All),
    )?;

    write!(stdout, "Claude Sessions (live) -- press q to quit\r\n")?;
    write!(stdout, "\r\n")?;
    for line in table.lines() {
        write!(stdout, "{line}\r\n")?;
    }
    stdout.flush()?;
    Ok(())
}

fn enter_tui() -> anyhow::Result<CleanupGuard> {
    terminal::enable_raw_mode()?;
    execute!(std::io::stdout(), EnterAlternateScreen, cursor::Hide)?;
    Ok(CleanupGuard)
}

fn start_watcher() -> anyhow::Result<(notify::RecommendedWatcher, mpsc::Receiver<notify::Event>)> {
    let session_file = session::state_file_path();
    let watch_dir = session_file
        .parent()
        .ok_or_else(|| anyhow::anyhow!("cannot determine parent directory of session file"))?
        .to_path_buf();

    let (tx, rx) = mpsc::channel();
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        if let Ok(event) = res {
            let _ = tx.send(event);
        }
    })?;
    watcher.watch(&watch_dir, RecursiveMode::NonRecursive)?;
    Ok((watcher, rx))
}

fn should_quit(key: &crossterm::event::KeyEvent) -> bool {
    matches!(key.code, KeyCode::Char('q') | KeyCode::Esc)
        || (key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL))
}

fn has_file_changes(rx: &mpsc::Receiver<notify::Event>) -> bool {
    let mut changed = false;
    while let Ok(event) = rx.try_recv() {
        if is_session_file_event(&event) {
            changed = true;
        }
    }
    changed
}

pub fn watch() -> anyhow::Result<()> {
    let _guard = enter_tui()?;
    let (_watcher, rx) = start_watcher()?;

    render()?;
    let mut last_render = Instant::now();

    loop {
        if crossterm::event::poll(Duration::from_secs(1))?
            && let Event::Key(key) = crossterm::event::read()?
            && should_quit(&key)
        {
            break;
        }

        let timestamp_stale = last_render.elapsed() >= Duration::from_secs(30);

        if has_file_changes(&rx) || timestamp_stale {
            render()?;
            last_render = Instant::now();
        }
    }

    Ok(())
}
