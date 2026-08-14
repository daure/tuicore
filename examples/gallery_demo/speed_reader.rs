use ratatui::layout::{Constraint, Direction, Layout, Rect};
use tuicore::{Button, ChildKey, DialogHost, SpeedReader};

use crate::Msg;

const SPEED_READER_MARKDOWN: &str = r#"# Read at the speed of thought

Speed reading works best when the interface stays **quiet** and your eyes stay anchored.

- Keep one recognition point in the center.
- Pause naturally after punctuation and between ideas.
- Use h and l to revisit individual words.
  - Use j and k to move between Markdown blocks.
- Follow [meaningful links](https://example.com) without reading their destinations.

> Markdown context remains visible while each word gets the spotlight.
"#;

const SPEED_READER_PLAIN: &str = "Speed reading keeps your eyes anchored while words arrive one at a time. This plain text example has no structural markers, no headings, and no Markdown styling. Pause whenever you need a moment, then continue at your own pace.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SpeedReaderExample {
    Markdown,
    Plain,
}

impl SpeedReaderExample {
    fn title(self) -> &'static str {
        match self {
            Self::Markdown => "Markdown example",
            Self::Plain => "Plain text example",
        }
    }

    fn button_label(self) -> &'static str {
        match self {
            Self::Markdown => "Open Markdown example",
            Self::Plain => "Open plain text example",
        }
    }

    fn hotkey(self) -> &'static str {
        match self {
            Self::Markdown => "sm",
            Self::Plain => "sp",
        }
    }

    fn reader(self) -> SpeedReader {
        match self {
            Self::Markdown => SpeedReader::markdown(SPEED_READER_MARKDOWN),
            Self::Plain => SpeedReader::new(SPEED_READER_PLAIN),
        }
        .title(self.title())
    }
}

pub(crate) fn speed_reader_buttons() -> [Button<Msg>; 2] {
    [SpeedReaderExample::Markdown, SpeedReaderExample::Plain].map(|example| {
        Button::new(example.button_label())
            .hotkey(example.hotkey())
            .on_press(move || Msg::SpeedReaderOpened(example))
    })
}

pub(crate) fn gallery_speed_reader(example: SpeedReaderExample) -> DialogHost<SpeedReader, Msg> {
    example.reader().dialog(Msg::SpeedReaderClosed)
}

pub(crate) fn speed_reader_button_areas(area: Rect) -> [Rect; 2] {
    let [_, markdown, _, plain, _] = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Fill(1),
        ])
        .areas(area);
    [markdown, plain]
}

pub(crate) fn speed_reader_child_key(index: usize) -> ChildKey {
    ChildKey::new(format!("speed-reader-open-{index}"))
}

pub(crate) fn speed_reader_index(key: &ChildKey) -> Option<usize> {
    key.as_str()
        .strip_prefix("speed-reader-open-")?
        .parse()
        .ok()
        .filter(|index| *index < 2)
}

pub(crate) fn speed_reader_route(
    route: &tuicore::EventRoute,
) -> Option<(usize, tuicore::EventRoute)> {
    let first = route.path.first()?;
    let index = speed_reader_index(first)?;
    Some((index, tuicore::EventRoute::new(route.path.without_first())))
}
