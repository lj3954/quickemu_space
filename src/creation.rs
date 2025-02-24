// SPDX-License-Identifier: GPL-3.0-only

mod download;
mod options;

use std::fmt::Display;

use cosmic::{
    app::command::Task,
    iced::{
        alignment::{Horizontal, Vertical},
        Length,
    },
    theme,
    widget::{self, combo_box, icon},
    Apply, Element,
};
use quickget_core::{data_structures::OS, ConfigSearch};

pub struct State {
    os_list: Vec<OS>,
    page: Page,
}

impl State {
    pub fn new() -> (Self, Task<crate::app::Message>) {
        let task = Task::perform(
            async { ConfigSearch::new().await.map(|x| x.into_os_list()) },
            |x| {
                match x {
                    Ok(os_list) => crate::app::Message::Creation(Message::OSList(os_list)),
                    Err(e) => crate::app::Message::Creation(Message::Error(e.to_string())),
                }
                .into()
            },
        );
        (
            Self {
                os_list: vec![],
                page: Page::default(),
            },
            task,
        )
    }
    pub fn update(&mut self, msg: Message) -> Task<crate::app::Message> {
        match msg {
            Message::OSList(os_list) => {
                self.os_list = os_list;
                self.page = Page::SelectOS;
            }
            Message::SelectedOS(os) => {
                self.page = Page::Options(options::OptionSelection::new(os));
            }
            Message::Options(msg) => match self.page {
                Page::Options(ref mut options) => return options.update(msg),
                _ => panic!("Options message while not being on options page"),
            },
            Message::Error(e) => {
                self.page = Page::Error(e);
            }
            Message::ChangePage(page) => {
                self.page = *page;
            }
            Message::StartDownloads(vm_name) => match self.page {
                Page::Options(ref mut options) => {
                    let instance = match options.to_instance(&vm_name) {
                        Ok(instance) => instance,
                        Err(e) => {
                            self.page = Page::Error(e);
                            return Task::none();
                        }
                    };
                    let (download_status, task) = download::DownloadStatus::new(instance);
                    self.page = Page::Download(download_status);
                    return task;
                }
                _ => panic!("Download message while not being on download page"),
            },
            Message::Download(msg) => match self.page {
                Page::Download(ref mut download) => return download.update(msg),
                _ => panic!("Download message while not being on download page"),
            },
        }
        Task::none()
    }
    pub fn view(&self) -> Element<crate::app::Message> {
        match self.page {
            Page::Loading => widget::text("Loading")
                .apply(widget::container)
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(Horizontal::Center)
                .align_y(Vertical::Center)
                .into(),
            Page::SelectOS => {
                let mut list_column = widget::list_column().style(theme::Container::ContextDrawer);
                for os in &self.os_list {
                    let mut row = widget::row();

                    let homepage_button = os.homepage.clone().map(|homepage| {
                        widget::button::icon(icon::from_name("go-home-symbolic"))
                            .on_press(crate::app::Message::LaunchUrl(homepage))
                            .tooltip(format!("Visit {} homepage", os.pretty_name))
                    });
                    row = row.push_maybe(homepage_button);

                    let button = widget::button::text(os.pretty_name.clone())
                        .on_press(Message::SelectedOS(os.to_owned()).into())
                        .width(Length::Fill);
                    row = row.push(button);

                    list_column = list_column.add(row);
                }
                widget::scrollable(list_column).into()
            }
            Page::Options(ref options) => options.view(),
            Page::Download(ref download) => download.view(),
            Page::Error(ref e) => widget::text(e).into(),
            _ => todo!(),
        }
    }
}

#[derive(Debug, Clone)]
struct SelectableComboBox<T: Display + Clone + PartialEq> {
    state: combo_box::State<T>,
    selected: Option<T>,
}

impl<T: Display + Clone + PartialEq> SelectableComboBox<T> {
    fn new(entries: impl IntoIterator<Item = T>, selected: Option<T>) -> Self {
        Self {
            state: combo_box::State::new(entries.into_iter().collect()),
            selected,
        }
    }
    fn new_empty() -> Self {
        Self {
            state: combo_box::State::new(vec![]),
            selected: None,
        }
    }
    fn try_select(&mut self, selected: T) {
        if self.state.options().contains(&selected) {
            self.selected = Some(selected);
        }
    }
    fn select(&mut self, selected: Option<T>) {
        self.selected = selected;
    }
    fn selected(&self) -> Option<&T> {
        self.selected.as_ref()
    }
    fn set_values(&mut self, new_entries: impl IntoIterator<Item = T>) {
        let vec: Vec<T> = new_entries.into_iter().collect();
        if self
            .selected
            .as_ref()
            .is_some_and(|selected| !vec.contains(selected))
        {
            self.selected = None;
        }
        self.state = combo_box::State::new(vec);
    }
    fn widget<F: Fn(T) -> crate::app::Message + 'static>(
        &self,
        placeholder: &str,
        on_selected: F,
    ) -> Option<widget::ComboBox<'_, T, crate::app::Message, cosmic::Theme, cosmic::iced::Renderer>>
    {
        (!self.is_empty()).then(|| {
            widget::combo_box(
                &self.state,
                placeholder,
                self.selected.as_ref(),
                on_selected,
            )
        })
    }
    fn is_empty(&self) -> bool {
        self.state.options().is_empty()
    }
}

#[derive(Clone, Debug, Default)]
pub(super) enum Page {
    #[default]
    Loading,
    SelectOS,
    Options(options::OptionSelection),
    Download(download::DownloadStatus),
    Docker,
    Complete,
    Error(String),
}

#[derive(Clone, Debug)]
pub(super) enum Message {
    OSList(Vec<OS>),
    SelectedOS(OS),
    Options(options::Message),
    StartDownloads(String),
    Download(download::Message),
    Error(String),
    ChangePage(Box<Page>),
}

impl From<Message> for crate::app::Message {
    fn from(value: Message) -> Self {
        crate::app::Message::Creation(value)
    }
}
