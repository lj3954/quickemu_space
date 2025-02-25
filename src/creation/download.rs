use std::{borrow::Cow, fs::File, io::Write};

use cosmic::{
    app::command::Task,
    iced::{futures, task, Alignment, Length, Pixels},
    widget, Element,
};
use futures_util::StreamExt;
use quickget_core::{QGDownload, QuickgetInstance};
use size::Size;

#[derive(Debug, Clone)]
pub struct DownloadStatus {
    instance: QuickgetInstance,
    downloads: Vec<Download>,
    handle: task::Handle,
}

impl DownloadStatus {
    pub(super) fn new(mut instance: QuickgetInstance) -> (Self, Task<crate::app::Message>) {
        let client = reqwest::Client::new();
        let downloads = instance.get_downloads();
        let (downloads, tasks): (Vec<_>, Vec<_>) = downloads
            .into_iter()
            .enumerate()
            .map(|(id, d)| Download::new(d, client.clone(), id))
            .unzip();

        let (task, handle) = Task::abortable(Task::batch(tasks));

        (
            Self {
                instance,
                downloads,
                handle,
            },
            task,
        )
    }

    pub(super) fn update(&mut self, msg: Message) -> Task<crate::app::Message> {
        match msg {
            Message::CancelDownloads => {
                self.handle.abort();
                return Task::perform(
                    async move {
                        crate::app::Message::from(super::Message::ChangePage(
                            super::Page::SelectOS.into(),
                        ))
                    },
                    |msg| msg.into(),
                );
            }
            Message::Finalize => {
                let finalize_page = Task::perform(
                    async move {
                        crate::app::Message::from(super::Message::ChangePage(
                            super::Page::Finalizing.into(),
                        ))
                    },
                    |msg| msg.into(),
                );

                let instance = self.instance.clone();
                let finalize = Task::perform(
                    async move {
                        let config_file_path = instance.get_config_file_path().to_owned();

                        let finalize_result =
                            tokio::task::spawn_blocking(move || instance.create_config())
                                .await
                                .expect("Couldn't spawn thread");
                        crate::app::Message::from(match finalize_result {
                            Ok(_) => super::Message::FinalizedConfigPath(config_file_path),
                            Err(e) => super::Message::Error(format!("Error creating config: {e}")),
                        })
                    },
                    |msg| msg.into(),
                );

                return finalize_page.chain(finalize);
            }
            Message::Specific(SpecificDownloadMessage { id, msg }) => {
                let download = self
                    .downloads
                    .get_mut(id)
                    .expect("Specified download somehow does not exist in the vector");
                match msg {
                    DownloadMessage::Done => download.done = true,
                    DownloadMessage::GotTotalSize(size) => download.total_size = Some(size),
                    DownloadMessage::AddedChunk(size) => download.current_size += size,
                    DownloadMessage::Error(e) => {
                        return Task::perform(
                            async move { crate::app::Message::from(super::Message::Error(e)) },
                            |msg| msg.into(),
                        )
                    }
                }
            }
        }
        Task::none()
    }

    pub(super) fn view(&self) -> Element<crate::app::Message> {
        let download_list = self
            .downloads
            .iter()
            .fold(widget::list_column(), |list, download| {
                list.add(download.view())
            });

        let nav_row = {
            let mut row = widget::row();

            let cancel =
                widget::button::suggested("Cancel").on_press(Message::CancelDownloads.into());
            row = row.push(cancel);

            let next = widget::button::suggested("Next");
            let next = if self.downloads.iter().all(|dl| dl.done) {
                next.on_press(Message::Finalize.into())
            } else {
                next
            };

            row.push(
                widget::container(next)
                    .align_right(Length::Shrink)
                    .width(Length::Fill),
            )
        };

        widget::list_column()
            .add(download_list)
            .add(widget::vertical_space())
            .add(nav_row)
            .into()
    }
}

#[derive(Debug, Clone)]
pub(crate) enum Message {
    CancelDownloads,
    Finalize,
    Specific(SpecificDownloadMessage),
}

#[derive(Debug, Clone)]
pub(crate) struct SpecificDownloadMessage {
    id: usize,
    msg: DownloadMessage,
}

#[derive(Debug, Clone)]
enum DownloadMessage {
    GotTotalSize(u64),
    AddedChunk(u64),
    Done,
    Error(String),
}

impl From<Message> for crate::app::Message {
    fn from(value: Message) -> Self {
        crate::app::Message::Creation(super::Message::Download(value))
    }
}

#[derive(Debug, Clone)]
struct Download {
    name: String,
    current_size: u64,
    total_size: Option<u64>,
    done: bool,
}

#[derive(Debug, derive_more::From)]
enum DownloadError {
    Reqwest(reqwest::Error),
    Io(tokio::io::Error),
}

impl std::error::Error for DownloadError {}
impl std::fmt::Display for DownloadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DownloadError::Reqwest(e) => write!(f, "Error while downloading: {}", e),
            DownloadError::Io(e) => write!(f, "Error while writing to file: {}", e),
        }
    }
}

impl Download {
    fn new(
        source: QGDownload,
        client: reqwest::Client,
        id: usize,
    ) -> (Self, Task<crate::app::Message>) {
        let name = source
            .path
            .file_name()
            .unwrap_or(source.path.as_os_str())
            .to_string_lossy()
            .to_string();

        let spawn_request = cosmic::Task::perform(
            async move {
                let mut request = client.get(source.url);
                if let Some(headers) = source.headers {
                    request = request.headers(headers);
                }
                let response = request.send().await?;
                let file = File::create(source.path)?;

                Ok::<_, DownloadError>((response, file))
            },
            |r| r,
        );

        let task = spawn_request.then(move |r| {
            Task::run(
                'task: {
                    let (response, mut file) = match r {
                        Ok(r) => r,
                        Err(e) => {
                            break 'task futures::stream::once(async move { Err(e) }).boxed();
                        }
                    };

                    let total_size_msg =
                        DownloadMessage::GotTotalSize(response.content_length().unwrap_or(0));
                    let total_size_msg = futures::stream::once(async move { Ok(total_size_msg) });

                    let dl_stream = response.bytes_stream().map(move |chunk| {
                        let chunk = chunk.map_err(DownloadError::Reqwest)?;
                        file.write_all(&chunk)?;
                        Ok::<_, DownloadError>(DownloadMessage::AddedChunk(chunk.len() as u64))
                    });

                    let done = futures::stream::once(async { Ok(DownloadMessage::Done) });

                    total_size_msg.chain(dl_stream).chain(done).boxed()
                },
                move |msg| {
                    let msg = msg.unwrap_or_else(|e| DownloadMessage::Error(e.to_string()));
                    crate::app::Message::from(Message::Specific(SpecificDownloadMessage {
                        id,
                        msg,
                    }))
                    .into()
                },
            )
        });

        (
            Self {
                name,
                current_size: 0,
                total_size: None,
                done: false,
            },
            task,
        )
    }

    fn view(&self) -> Element<crate::app::Message> {
        let status_text = if let Some(total_size) = self.total_size {
            Cow::Owned(if total_size == 0 {
                format!("{} / ??", Size::from_bytes(self.current_size))
            } else {
                format!(
                    "{} / {} ({:.2}%)",
                    Size::from_bytes(self.current_size),
                    Size::from_bytes(total_size),
                    self.current_size as f64 / total_size as f64 * 100.0
                )
            })
        } else {
            Cow::Borrowed("Download starting")
        };

        let widgets = vec![
            Element::from(widget::text(self.name.as_str())),
            widget::horizontal_space().width(Pixels(5.0)).into(),
            widget::progress_bar(
                0.0..=self.total_size.unwrap_or(0) as f32,
                self.current_size as f32,
            )
            .into(),
            widget::horizontal_space().width(Pixels(5.0)).into(),
            widget::text(status_text)
                .class(cosmic::style::Text::Accent)
                .into(),
        ];

        widget::flex_row(widgets)
            .justify_items(Alignment::Center)
            .into()
    }
}
