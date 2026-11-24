//! What basemapper decided for the user, and what it could not do: notes
//! carried on a view and its rendering (docs/design/api-review.md §4.8).
//!
//! The level decides how R and Python surface a note: *decided* never
//! interrupts (shown on request), *adjusted* is a message once per kind per
//! session, *problem* is a warning every time.
//!
//! A render collects its notes in a [`Collector`]. Code anywhere in the
//! pipeline calls [`report`], which adds to the current collector (and logs,
//! as before). The current collector is set around every poll of a future
//! wrapped with [`Collector::scope`] or [`propagate`], and around each worker
//! of [`crate::vector::memo::par_map`], so it follows the work across tokio
//! tasks and threads without being passed through every function.

use std::cell::RefCell;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

/// How much a note matters to the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    /// A value the user left open, chosen by basemapper.
    Decided,
    /// Something the user asked for was changed, or settled for less.
    Adjusted,
    /// The output is incomplete or likely not what was meant.
    Problem,
}

impl Level {
    /// `"decided"`, `"adjusted"` or `"problem"`.
    pub fn name(self) -> &'static str {
        match self {
            Level::Decided => "decided",
            Level::Adjusted => "adjusted",
            Level::Problem => "problem",
        }
    }
}

/// One note: its level, a stable kind (for code to match on), a message for
/// people, and the details of a note that happened several times.
#[derive(Debug, Clone, PartialEq)]
pub struct Note {
    pub level: Level,
    /// A stable identifier, e.g. `"effective_dpi"`, `"tile_skipped"`.
    pub kind: &'static str,
    pub message: String,
    /// Each occurrence's message, when the kind happened more than once (the
    /// first [`MAX_DETAILS`] distinct ones).
    pub details: Vec<String>,
    /// How many times it happened.
    pub count: u32,
}

/// The most occurrences of one kind a note lists.
pub const MAX_DETAILS: usize = 20;

impl Note {
    pub fn new(level: Level, kind: &'static str, message: impl Into<String>) -> Self {
        Note { level, kind, message: message.into(), details: Vec::new(), count: 1 }
    }
}

/// The notes of one render, shared by the tasks and threads doing it.
#[derive(Debug, Clone, Default)]
pub struct Collector(Arc<Mutex<Vec<Note>>>);

thread_local! {
    static CURRENT: RefCell<Option<Collector>> = const { RefCell::new(None) };
}

/// Restores the previous current collector, also on a panic.
struct Restore(Option<Collector>);

impl Drop for Restore {
    fn drop(&mut self) {
        let previous = self.0.take();
        CURRENT.with(|c| *c.borrow_mut() = previous);
    }
}

fn enter<R>(collector: Option<&Collector>, f: impl FnOnce() -> R) -> R {
    let previous = CURRENT.with(|c| c.replace(collector.cloned()));
    let _restore = Restore(previous);
    f()
}

impl Collector {
    pub fn new() -> Self {
        Collector::default()
    }

    /// The collector of the render this code is part of, if any.
    pub fn current() -> Option<Collector> {
        CURRENT.with(|c| c.borrow().clone())
    }

    /// Run `f` with this as the current collector.
    pub fn enter<R>(&self, f: impl FnOnce() -> R) -> R {
        enter(Some(self), f)
    }

    /// `future`, with this as the current collector whenever it runs.
    pub fn scope<F: Future>(&self, future: F) -> Scoped<F> {
        Scoped { collector: Some(self.clone()), future: Box::pin(future) }
    }

    /// Add a note. A kind already noted is counted and its new message kept
    /// in the details, so a hundred failed tiles are one note.
    pub fn add(&self, note: Note) {
        let mut notes = self.0.lock().unwrap_or_else(|e| e.into_inner());
        match notes.iter_mut().find(|n| n.kind == note.kind && n.level == note.level) {
            Some(n) => {
                if n.details.is_empty() {
                    n.details.push(n.message.clone());
                }
                n.count += 1;
                if n.details.len() < MAX_DETAILS && !n.details.contains(&note.message) {
                    n.details.push(note.message);
                }
            }
            None => notes.push(note),
        }
    }

    /// The notes so far, in the order their kinds first happened; a kind
    /// that happened several times says how often.
    pub fn notes(&self) -> Vec<Note> {
        let notes = self.0.lock().unwrap_or_else(|e| e.into_inner());
        notes
            .iter()
            .map(|n| {
                let mut n = n.clone();
                if n.count > 1 {
                    n.message = format!("{} (and {} more like it)", n.message, n.count - 1);
                }
                n
            })
            .collect()
    }
}

/// A future run with a collector current (see [`Collector::scope`]).
pub struct Scoped<F> {
    collector: Option<Collector>,
    future: Pin<Box<F>>,
}

impl<F: Future> Future for Scoped<F> {
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<F::Output> {
        let this = self.get_mut();
        enter(this.collector.as_ref(), || this.future.as_mut().poll(cx))
    }
}

/// `future` with the current collector, for a task spawned by a render.
pub fn propagate<F: Future>(future: F) -> Scoped<F> {
    Scoped { collector: Collector::current(), future: Box::pin(future) }
}

/// `f` with the current collector, for a thread started by a render.
pub fn propagate_fn<R>(f: impl FnOnce() -> R + Send) -> impl FnOnce() -> R + Send {
    let collector = Collector::current();
    move || enter(collector.as_ref(), f)
}

/// Note `message` in the current render, if any, and log it (`decided` at
/// info level, the others as warnings).
pub fn report(level: Level, kind: &'static str, message: impl Into<String>) {
    let message = message.into();
    match level {
        Level::Decided => log::info!("{message}"),
        Level::Adjusted | Level::Problem => log::warn!("{message}"),
    }
    if let Some(c) = Collector::current() {
        c.add(Note::new(level, kind, message));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notes_follow_the_work_and_merge_by_kind() {
        let c = Collector::new();
        report(Level::Problem, "tile_skipped", "outside any render: not collected");
        c.enter(|| {
            report(Level::Problem, "tile_skipped", "tile 1/0/0: timeout");
            // A worker thread and a spawned task report into the same render.
            std::thread::scope(|s| {
                s.spawn(propagate_fn(|| report(Level::Problem, "tile_skipped", "tile 1/1/0: 404")));
            });
            let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(2).build().unwrap();
            rt.block_on(async {
                tokio::spawn(propagate(async { report(Level::Adjusted, "tiles_coarsened", "coarser tiles") }))
                    .await
                    .unwrap();
            });
        });
        report(Level::Problem, "tile_skipped", "after the render: not collected");
        let notes = c.notes();
        assert_eq!(notes.len(), 2, "{notes:?}");
        assert_eq!((notes[0].kind, notes[0].count), ("tile_skipped", 2));
        assert_eq!(notes[0].details, ["tile 1/0/0: timeout", "tile 1/1/0: 404"]);
        assert!(notes[0].message.ends_with("(and 1 more like it)"), "{}", notes[0].message);
        assert_eq!((notes[1].kind, notes[1].level), ("tiles_coarsened", Level::Adjusted));
        assert!(Collector::current().is_none());
    }
}
