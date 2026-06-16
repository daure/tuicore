mod app;
mod dispatcher;
mod event_source;
mod focus;
mod layout;
mod renderer;
mod scheduler;
mod terminal;

pub use app::{TreeApp, run};
pub use dispatcher::{DispatchEffects, TreeDispatcher};
pub use event_source::EventSource;
pub use focus::{FocusManager, FocusTransition};
pub use layout::LayoutEngine;
pub use renderer::Renderer;
pub use scheduler::Scheduler;
pub use terminal::TerminalGuard;

pub type Result<T> = std::io::Result<T>;
