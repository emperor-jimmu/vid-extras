// TUI module - Terminal UI with split-pane logs per thread

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;
use ratatui::prelude::Color;
use ratatui::style::Style;
use ratatui::symbols::block::FULL;
use ratatui::widgets::{Block, Paragraph};
use ratatui::Frame;
use std::collections::HashMap;
use std::io;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const MAX_LINES_PER_PANE: usize = 1000;
const PROGRESS_BAR_HEIGHT: usize = 3;

pub struct TuiState {
    active: Arc<AtomicBool>,
    requested_stop: Arc<AtomicBool>,
    thread_count: Arc<AtomicUsize>,
    current_item: Arc<Mutex<String>>,
    total_items: Arc<AtomicUsize>,
    processed_items: Arc<AtomicUsize>,
    thread_logs: Arc<Mutex<HashMap<usize, Vec<String>>>>,
    system_status: Arc<Mutex<String>>,
    active_items: Arc<Mutex<Vec<String>>>,
}

impl Clone for TuiState {
    fn clone(&self) -> Self {
        Self {
            active: self.active.clone(),
            requested_stop: self.requested_stop.clone(),
            thread_count: self.thread_count.clone(),
            current_item: self.current_item.clone(),
            total_items: self.total_items.clone(),
            processed_items: self.processed_items.clone(),
            thread_logs: self.thread_logs.clone(),
            system_status: self.system_status.clone(),
            active_items: self.active_items.clone(),
        }
    }
}

impl Default for TuiState {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(clippy::new_without_default)]
impl TuiState {
    pub fn new() -> Self {
        Self {
            active: Arc::new(AtomicBool::new(false)),
            requested_stop: Arc::new(AtomicBool::new(false)),
            thread_count: Arc::new(AtomicUsize::new(0)),
            current_item: Arc::new(Mutex::new(String::new())),
            total_items: Arc::new(AtomicUsize::new(0)),
            processed_items: Arc::new(AtomicUsize::new(0)),
            thread_logs: Arc::new(Mutex::new(HashMap::new())),
            system_status: Arc::new(Mutex::new(String::new())),
            active_items: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn add_active_item(&self, item: &str) {
        if let Ok(mut items) = self.active_items.lock() {
            if !items.contains(&item.to_string()) {
                items.push(item.to_string());
            }
        }
    }

    pub fn remove_active_item(&self, item: &str) {
        if let Ok(mut items) = self.active_items.lock() {
            items.retain(|i| i != item);
        }
    }

    pub fn get_active_items(&self) -> Vec<String> {
        if let Ok(items) = self.active_items.lock() {
            items.clone()
        } else {
            Vec::new()
        }
    }

    pub fn set_thread_count(&self, count: usize) {
        self.thread_count.store(count, Ordering::SeqCst);
    }

    pub fn set_total_items(&self, total: usize) {
        self.total_items.store(total, Ordering::SeqCst);
    }

    pub fn start(&self) {
        self.active.store(true, Ordering::SeqCst);
    }

    pub fn stop(&self) {
        self.active.store(false, Ordering::SeqCst);
    }

    pub fn is_stop_requested(&self) -> bool {
        self.requested_stop.load(Ordering::SeqCst)
    }

    pub fn request_stop(&self) {
        self.requested_stop.store(true, Ordering::SeqCst);
    }

    pub fn set_current_item(&self, item: &str) {
        if let Ok(mut current) = self.current_item.lock() {
            *current = item.to_string();
        }
    }

    pub fn set_system_status(&self, status: &str) {
        if let Ok(mut s) = self.system_status.lock() {
            *s = status.to_string();
        }
    }

    pub fn increment_processed(&self) {
        self.processed_items.fetch_add(1, Ordering::SeqCst);
    }

    pub fn log(&self, thread_id: usize, line: &str) {
        if !self.active.load(Ordering::SeqCst) {
            return;
        }
        if let Ok(mut logs) = self.thread_logs.lock() {
            let thread_logs = logs.entry(thread_id).or_insert_with(Vec::new);
            thread_logs.push(line.to_string());
            if thread_logs.len() > MAX_LINES_PER_PANE {
                thread_logs.remove(0);
            }
        }
    }

    pub fn log_if_active(&self, thread_id: usize, line: &str) {
        self.log(thread_id, line);
    }

    fn get_thread_id(&self) -> usize {
        let thread_id_counter = Arc::new(AtomicUsize::new(0));
        let id = thread_id_counter.fetch_add(1, Ordering::SeqCst);
        id % self.thread_count.load(Ordering::SeqCst).max(1)
    }

    pub fn capture_log(&self, line: &str) {
        if !self.active.load(Ordering::SeqCst) {
            return;
        }
        let thread_id = self.get_thread_id();
        self.log(thread_id, line);
    }

    fn get_progress(&self) -> f64 {
        let total = self.total_items.load(Ordering::SeqCst);
        let processed = self.processed_items.load(Ordering::SeqCst);
        if total == 0 {
            0.0
        } else {
            processed as f64 / total as f64
        }
    }

    fn render(&self, frame: &mut Frame) {
        let area = frame.area();
        let thread_count = self.thread_count.load(Ordering::SeqCst).max(1) as u16;

        let progress_height = PROGRESS_BAR_HEIGHT as u16;
        let total_height = area.height.saturating_sub(progress_height);

        if total_height == 0 {
            return;
        }

        let pane_height = total_height / thread_count.max(1);

        let mut pane_areas = Vec::new();
        for i in 0..thread_count {
            let y = area.y + (i * pane_height);
            let height = if i == thread_count - 1 {
                pane_height + (total_height - (pane_height * thread_count))
            } else {
                pane_height
            };
            pane_areas.push(Rect::new(area.x, y, area.width, height));
        }

        let logs = match self.thread_logs.lock() {
            Ok(logs) => logs,
            Err(_) => return,
        };

        for (idx, pane_area) in pane_areas.iter().enumerate() {
            if pane_area.height < 2 {
                continue;
            }

            let title = format!(" [Thread {}] ", idx + 1);
            let block = Block::default()
                .title(title.as_str())
                .borders(ratatui::widgets::Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan));

            let thread_logs = logs.get(&idx).map(|v| v.as_slice()).unwrap_or(&[]);

            let mut content = String::new();
            let line_count = (pane_area.height as usize).saturating_sub(2);
            let start = thread_logs.len().saturating_sub(line_count);
            for line in thread_logs.iter().skip(start) {
                content.push_str(line);
                content.push('\n');
            }

            let paragraph = Paragraph::new(content.as_str())
                .block(block)
                .style(Style::default().fg(Color::White));

            frame.render_widget(paragraph, *pane_area);
        }

        let progress_area = Rect::new(
            area.x,
            area.y + total_height,
            area.width,
            progress_height,
        );

        self.render_progress_bar(frame, progress_area);
    }

    fn render_progress_bar(&self, frame: &mut Frame, area: Rect) {
        let progress = self.get_progress();
        let total = self.total_items.load(Ordering::SeqCst);
        let processed = self.processed_items.load(Ordering::SeqCst);
        let total_threads = self.thread_count.load(Ordering::SeqCst);

        let active_items = self.get_active_items();
        let current_item = self.current_item.lock().map(|c| c.clone()).unwrap_or_default();

        let progress_bar_width = area.width.saturating_sub(2) as usize;
        let filled = ((progress * progress_bar_width as f64).round() as usize).min(progress_bar_width);
        let empty = progress_bar_width - filled;

        let bar: String = FULL
            .chars()
            .take(filled)
            .collect::<String>()
            .chars()
            .chain(" ".repeat(empty).chars())
            .collect();

        let system_status = self.system_status.lock().map(|s| s.clone()).unwrap_or_default();

        let status_text = if self.requested_stop.load(Ordering::SeqCst) {
            " STOPPING... ".to_string()
        } else if !system_status.is_empty() {
            format!(" {} ", system_status)
        } else {
            format!(" {:>5.1}% ", (progress * 100.0) as u32)
        };

        let items_display = if active_items.is_empty() {
            if current_item.is_empty() {
                String::new()
            } else if current_item.len() > 30 {
                format!("...{}", &current_item[current_item.len() - 30..])
            } else {
                current_item.clone()
            }
        } else {
            active_items.join(" | ")
        };

        let title = format!(
            " {} {}/{} | {} threads | {}",
            status_text,
            processed,
            total,
            total_threads,
            items_display
        );

        let border_color = if self.requested_stop.load(Ordering::SeqCst) {
            Color::Yellow
        } else if progress >= 1.0 {
            Color::Green
        } else {
            Color::Blue
        };

        let content_color = if self.requested_stop.load(Ordering::SeqCst) {
            Color::Yellow
        } else if progress >= 1.0 {
            Color::Green
        } else {
            Color::Cyan
        };

        let block = Block::default()
            .title(title.as_str())
            .borders(ratatui::widgets::Borders::ALL)
            .border_style(Style::default().fg(border_color));

        let paragraph = Paragraph::new(bar.as_str())
            .block(block)
            .style(Style::default().fg(content_color));

        frame.render_widget(paragraph, area);
    }
}

pub fn run_tui(state: Arc<TuiState>) {
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = match ratatui::Terminal::new(backend) {
        Ok(t) => t,
        Err(_) => return,
    };

    if terminal.autoresize().is_err() {
        return;
    }

    while state.active.load(Ordering::SeqCst) {
        if terminal.draw(|f| state.render(f)).is_err() {
            break;
        }

#[allow(clippy::collapsible_if)]
        if event::poll(Duration::from_millis(50)).unwrap_or(false) {
            if let Event::Key(key) = event::read().unwrap_or(Event::FocusGained) {
                if key.kind == KeyEventKind::Press
                    && (key.code == KeyCode::Char('q') || key.code == KeyCode::Esc)
{
                    state.request_stop();
                    break;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tui_state_creation() {
        let state = TuiState::new();
        assert!(!state.active.load(Ordering::SeqCst));
        assert!(!state.is_stop_requested());
    }

    #[test]
    fn test_tui_state_thread_count() {
        let state = TuiState::new();
        state.set_thread_count(4);
        assert_eq!(state.thread_count.load(Ordering::SeqCst), 4);
    }

    #[test]
    fn test_tui_state_total_items() {
        let state = TuiState::new();
        state.set_total_items(100);
        assert_eq!(state.total_items.load(Ordering::SeqCst), 100);
    }

    #[test]
    fn test_tui_state_progress_calculation() {
        let state = TuiState::new();
        state.set_total_items(50);
        state.increment_processed();
        state.increment_processed();
        assert_eq!(state.get_progress(), 0.04);
    }

    #[test]
    fn test_tui_state_stop_request() {
        let state = TuiState::new();
        assert!(!state.is_stop_requested());
        state.request_stop();
        assert!(state.is_stop_requested());
    }

    #[test]
    fn test_tui_state_current_item() {
        let state = TuiState::new();
        state.set_current_item("Test Movie (2020)");
        let current = state.current_item.lock().unwrap();
        assert_eq!(*current, "Test Movie (2020)");
    }
}