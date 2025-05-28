use hash_gui::prelude::*;
use iced::futures::{SinkExt, Stream};
use iced::widget::{
    Space, column, container, horizontal_rule, progress_bar, row, scrollable, text, text_input,
};
use iced::window::settings::PlatformSpecific;
use iced::{
    Alignment, Background, Border, Element, Length, Settings, Size, Subscription, Task, Theme,
    keyboard, window,
};
use sha2::{Digest, Sha256};
use std::io::BufReader;
use std::io::prelude::*;
use std::path::PathBuf;

fn main() -> iced::Result {
    tracing_subscriber::fmt::init();

    #[cfg(target_os = "windows")]
    let platform_specific = PlatformSpecific {
        drag_and_drop: true,
        ..PlatformSpecific::default()
    };

    #[cfg(not(target_os = "windows"))]
    let platform_specific = PlatformSpecific::default();

    iced::application(App::title, App::update, App::view)
        .subscription(App::subscription)
        .settings(Settings {
            antialiasing: false,
            ..Settings::default()
        })
        .window(window::Settings {
            size: Size::new(640.0, 480.0),
            min_size: Some(Size::new(640.0, 480.0)),
            platform_specific,
            ..window::Settings::default()
        })
        .theme(App::theme)
        .run()
}

#[derive(Debug, Clone)]
enum Message {
    CalculateProgress(Result<FileEntry, ()>),
    FileDropped(PathBuf),
    ClearHistory,
}

#[derive(Default)]
struct App {
    file_entries: Vec<FileEntry>,
}

impl App {
    fn title(&self) -> String {
        let progress = self
            .file_entries
            .iter()
            .fold(0f32, |progress_min, data| match data.state {
                FileEntryState::Idle => progress_min,
                FileEntryState::Calculating { progress } => {
                    if progress_min == 0f32 {
                        progress
                    } else {
                        progress_min.min(progress)
                    }
                }
                FileEntryState::Finished { .. } => progress_min,
            });
        match progress {
            0f32 => "Hash GUI".into(),
            _ => format!("{progress:.0}% - Hash GUI"),
        }
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::CalculateProgress(data) => match data {
                Ok(result) => self
                    .file_entries
                    .iter_mut()
                    .find(|data| data.pathname == result.pathname)
                    .map(|data| {
                        data.state = result.state;
                        Task::none()
                    })
                    .unwrap_or_else(Task::none),
                Err(_e) => Task::none(),
            },
            Message::FileDropped(pathname) => {
                info!(file_entries = ?self.file_entries);
                if self
                    .file_entries
                    .iter()
                    .all(|data| data.pathname != pathname)
                    && pathname.is_file()
                {
                    self.file_entries.push(FileEntry {
                        pathname,
                        state: FileEntryState::Idle,
                    });
                }
                Task::none()
            }
            Message::ClearHistory => {
                if self.file_entries.is_empty() {
                    iced::exit()
                } else {
                    self.file_entries.clear();
                    Task::none()
                }
            }
        }
    }

    fn subscription(&self) -> Subscription<Message> {
        let mut subscriptions = self
            .file_entries
            .iter()
            .filter(|data| match data.state {
                FileEntryState::Idle | FileEntryState::Calculating { .. } => true,
                FileEntryState::Finished { .. } => false,
            })
            .map(|data| {
                Subscription::run_with_id(data.pathname.clone(), App::hash(data.clone()))
                    .map(Message::CalculateProgress)
            })
            .collect::<Vec<_>>();

        subscriptions.push(iced::event::listen_with(|event, _status, _id| {
            if let iced::Event::Window(window::Event::FileDropped(path)) = event {
                Some(Message::FileDropped(path))
            } else {
                None
            }
        }));

        subscriptions.push(iced::event::listen_with(|event, _status, _id| {
            if let iced::Event::Keyboard(keyboard::Event::KeyReleased {
                key: keyboard::Key::Named(keyboard::key::Named::Escape),
                ..
            }) = event
            {
                Some(Message::ClearHistory)
            } else {
                None
            }
        }));

        Subscription::batch(subscriptions)
    }

    fn selectable_text_style(theme: &Theme, _status: text_input::Status) -> text_input::Style {
        let palette = theme.extended_palette();

        // tweaks the default active style.
        text_input::Style {
            background: Background::Color(palette.background.base.color),
            border: Border::default(),
            icon: palette.background.weak.text,
            placeholder: palette.background.strong.color,
            value: palette.background.base.text,
            selection: palette.primary.weak.color,
        }
    }

    fn selectable_text_result_style(
        &self,
        index: usize,
        theme: &Theme,
        _status: text_input::Status,
    ) -> text_input::Style {
        let palette = theme.extended_palette();

        let background = match self.file_entries.first() {
            Some(FileEntry {
                state: FileEntryState::Finished { hash },
                ..
            }) => match self.file_entries.get(index) {
                None
                | Some(FileEntry {
                    state: FileEntryState::Idle,
                    ..
                })
                | Some(FileEntry {
                    state: FileEntryState::Calculating { .. },
                    ..
                }) => Background::Color(palette.background.base.color),
                Some(FileEntry {
                    state: FileEntryState::Finished { hash: other_hash },
                    ..
                }) if hash == other_hash => Background::Color(palette.success.base.color),
                Some(_) => Background::Color(palette.danger.base.color),
            },
            _ => Background::Color(palette.background.base.color),
        };

        // tweaks the default active style.
        text_input::Style {
            background,
            border: Border::default(),
            icon: palette.background.weak.text,
            placeholder: palette.background.strong.color,
            value: palette.background.base.text,
            selection: palette.primary.weak.color,
        }
    }

    fn view(&self) -> Element<Message> {
        if self.file_entries.is_empty() {
            return container(column([
                row([
                    text("Calculate").into(),
                    Space::with_width(4).into(),
                    text("Drop files here")
                        .color(self.theme().extended_palette().primary.strong.color)
                        .into(),
                ])
                .into(),
                row([
                    text("Clear/Exit").into(),
                    Space::with_width(4).into(),
                    text(if cfg!(target_os = "macos") {
                        "⎋"
                    } else {
                        "Esc"
                    })
                    .color(self.theme().extended_palette().primary.strong.color)
                    .into(),
                ])
                .into(),
            ]))
            .center(Length::Fill)
            .into();
        }

        let mut children = vec![];
        for (i, data) in self.file_entries.iter().enumerate() {
            if 0 < i {
                children.push(horizontal_rule(8).into());
            }

            children.push(
                row([
                    text("pathname: ").into(),
                    text_input("", &data.pathname.display().to_string())
                        .size(12)
                        .style(Self::selectable_text_style)
                        .into(),
                ])
                .into(),
            );

            let progress = match &data.state {
                FileEntryState::Idle => "",
                FileEntryState::Calculating { progress } => &format!("{:.0}%", progress),
                FileEntryState::Finished { .. } => "100%",
            };
            children.push(
                row([
                    text("progress: ").into(),
                    text_input("", progress)
                        .size(12)
                        .style(Self::selectable_text_style)
                        .into(),
                ])
                .into(),
            );
            children.push(
                row([
                    text("SHA256: ").into(),
                    match data.state {
                        FileEntryState::Idle => progress_bar(0.0..=100.0, 0.0).height(16).into(),
                        FileEntryState::Calculating { progress } => {
                            progress_bar(0.0..=100.0, progress).height(16).into()
                        }
                        FileEntryState::Finished { .. } => text_input(
                            "",
                            match &data.state {
                                FileEntryState::Finished { hash } => hash,
                                FileEntryState::Idle | FileEntryState::Calculating { .. } => "",
                            },
                        )
                        .size(12)
                        .style(move |theme, status| {
                            if i == 0 {
                                Self::selectable_text_style(theme, status)
                            } else {
                                self.selectable_text_result_style(i, theme, status)
                            }
                        })
                        .into(),
                    },
                ])
                .align_y(Alignment::Center)
                .into(),
            );
        }
        scrollable(column(children)).into()
    }

    fn theme(&self) -> Theme {
        Theme::default()
    }

    fn hash(entry: FileEntry) -> impl Stream<Item = Result<FileEntry, ()>> {
        iced::stream::try_channel(3, async move |mut output| {
            let output_inner = output.clone();
            let entry_inner = entry.clone();
            let hash = tokio::task::spawn_blocking(move || {
                let mut output = output_inner;
                let entry = entry_inner;
                let mut buf = [0u8; 8 * 1024];
                let mut reader =
                    BufReader::new(std::fs::File::open(&entry.pathname).expect("pathname.open"));

                let metadata =
                    std::fs::symlink_metadata(&entry.pathname).expect("pathname.symlink_metadata");
                let mut remain = metadata.len();
                let mut sum = 0u64;

                let mut hasher = Sha256::new();

                let mut progress = 0f32;

                match output.try_send(FileEntry {
                    pathname: entry.pathname.clone(),
                    state: FileEntryState::Calculating { progress },
                }) {
                    Err(e) if e.is_disconnected() => {
                        info!(?e, "disconnected (1st)");
                        return Err(());
                    }
                    Ok(_) | Err(_) => {}
                }

                while 0 < remain {
                    let read_size = (buf.len() as u64).min(remain) as usize;
                    reader
                        .read_exact(&mut buf[..read_size])
                        .expect("reader.read_exact");

                    Digest::update(&mut hasher, &buf[..read_size]);

                    remain -= read_size as u64;
                    sum += read_size as u64;

                    let new_progress = (sum as f32) / (metadata.len() as f32) * 100.0;
                    if progress < new_progress {
                        progress = new_progress;
                        match output.try_send(FileEntry {
                            pathname: entry.pathname.clone(),
                            state: FileEntryState::Calculating { progress },
                        }) {
                            Err(e) if e.is_disconnected() => {
                                info!(?e, "disconnected (2nd)");
                                return Err(());
                            }
                            Ok(_) | Err(_) => {}
                        }
                    }
                }

                Ok(format!("{:x}", hasher.finalize()))
            })
            .await
            .expect("spawn_blocking");

            if let Ok(hash) = hash {
                output
                    .send(FileEntry {
                        pathname: entry.pathname.clone(),
                        state: FileEntryState::Finished { hash },
                    })
                    .await
                    .ok();
            }
            Ok(())
        })
    }
}

#[derive(Debug, Clone)]
struct FileEntry {
    pathname: PathBuf,
    state: FileEntryState,
}

#[derive(Debug, Clone)]
enum FileEntryState {
    Idle,
    Calculating { progress: f32 },
    Finished { hash: String },
}
