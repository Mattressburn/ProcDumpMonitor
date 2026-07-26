//! Shared worker-thread plumbing for the three collector pages: collection
//! runs off the UI thread; progress lines flow back through an mpsc channel
//! drained on OnNotice (nwg::Notice senders are Send). One runner per page.

use native_windows_gui as nwg;
use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::sync::mpsc;

/// Sent by the worker when the run ends (Ok = run folder).
const DONE_PREFIX: &str = "\u{4}DONE\u{4}";

pub struct CollectRunner {
    pub notice: nwg::Notice,
    rx: RefCell<Option<mpsc::Receiver<String>>>,
    pub running: Cell<bool>,
    pub last_output: RefCell<Option<PathBuf>>,
}

pub fn build(parent: &nwg::Frame) -> CollectRunner {
    let mut notice = nwg::Notice::default();
    nwg::Notice::builder().parent(parent).build(&mut notice).expect("collect notice");
    CollectRunner {
        notice,
        rx: RefCell::new(None),
        running: Cell::new(false),
        last_output: RefCell::new(None),
    }
}

/// The progress sink handed to worker closures — feeds RunContext::start.
pub type Progress = Box<dyn FnMut(&str) + Send + 'static>;

impl CollectRunner {
    /// Start `work` on a worker thread. `work` gets a Send progress sink
    /// (pass it straight to RunContext::start) and returns the run folder.
    /// No-op while a run is already in flight.
    pub fn start<F>(&self, work: F) -> bool
    where
        F: FnOnce(Progress) -> Result<PathBuf, String> + Send + 'static,
    {
        if self.running.get() {
            return false;
        }
        let (tx, rx) = mpsc::channel::<String>();
        *self.rx.borrow_mut() = Some(rx);
        self.running.set(true);

        let sender = self.notice.sender();
        std::thread::spawn(move || {
            let tx_progress = tx.clone();
            let sender_progress = sender.clone();
            let progress: Progress = Box::new(move |s: &str| {
                let _ = tx_progress.send(s.to_string());
                sender_progress.notice();
            });
            let result = work(progress);
            let done = match result {
                Ok(dir) => format!("{DONE_PREFIX}OK\u{4}{}", dir.display()),
                Err(e) => format!("{DONE_PREFIX}ERR\u{4}{e}"),
            };
            let _ = tx.send(done);
            sender.notice();
        });
        true
    }

    /// Drain pending progress lines on OnNotice. Returns (new lines, and
    /// Some(result) once the run finished — Ok(run folder) / Err(message)).
    pub fn drain(&self) -> (Vec<String>, Option<Result<PathBuf, String>>) {
        let mut lines = Vec::new();
        let mut finished = None;
        if let Some(rx) = self.rx.borrow().as_ref() {
            while let Ok(line) = rx.try_recv() {
                if let Some(rest) = line.strip_prefix(DONE_PREFIX) {
                    let (kind, payload) = rest.split_once('\u{4}').unwrap_or(("ERR", rest));
                    if kind == "OK" {
                        let p = PathBuf::from(payload);
                        *self.last_output.borrow_mut() = Some(p.clone());
                        finished = Some(Ok(p));
                    } else {
                        finished = Some(Err(payload.to_string()));
                    }
                } else {
                    lines.push(line);
                }
            }
        }
        if finished.is_some() {
            self.running.set(false);
            *self.rx.borrow_mut() = None;
        }
        (lines, finished)
    }

    /// Open the last run folder in Explorer (or the Desktop fallback).
    pub fn open_last_output(&self) {
        let dir = self
            .last_output
            .borrow()
            .clone()
            .unwrap_or_else(crate::cli::default_collect_base);
        let _ = std::process::Command::new("explorer.exe").arg(dir).spawn();
    }
}
