// SPDX-License-Identifier: GPL-3.0-only

use std::{fmt::Display, path::PathBuf, sync::LazyLock};

use cosmic::{
    widget::{self, combo_box},
    Element,
};
use itertools::Itertools;
use quickemu_core::data::Arch;
use quickget_core::{
    data_structures::{Config, OS},
    QuickgetInstance,
};

static TOTAL_CPU_CORES: LazyLock<f64> =
    LazyLock::new(|| QuickgetInstance::get_total_cpu_cores() as f64);

pub struct State {
    os_list: Vec<OS>,
    page: Page,
    options: Option<OptionSelection>,
}

struct DisplayableOS(OS);

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
    fn selected(&self) -> Option<&T> {
        self.selected.as_ref()
    }
    fn set_values(&mut self, new_entries: impl IntoIterator<Item = T>) {
        let vec: Vec<T> = new_entries.into_iter().collect();
        if self
            .selected
            .as_ref()
            .is_some_and(|selected| vec.contains(selected))
        {
            self.selected = None;
        }
        self.state = combo_box::State::new(vec);
    }
    fn widget<F: Fn(T) -> crate::app::Message + 'static>(
        &self,
        placeholder: &str,
        on_selected: F,
    ) -> widget::ComboBox<'_, T, crate::app::Message, cosmic::Theme, cosmic::iced::Renderer> {
        widget::combo_box(
            &self.state,
            placeholder,
            self.selected.as_ref(),
            on_selected,
        )
    }
}

struct OptionSelection {
    selected_os: OS,
    release_list: SelectableComboBox<String>,
    edition_list: Option<SelectableComboBox<String>>,
    arch_list: SelectableComboBox<Arch>,
    cpu_cores: usize,
    ram: f64,
    directory: PathBuf,
}

impl OptionSelection {
    fn refresh(&mut self) {}

    fn refresh_releases(&mut self) {
        let releases = self
            .selected_os
            .releases
            .iter()
            .filter(|config| {
                self.arch_list
                    .selected()
                    .is_none_or(|arch| arch == &config.arch)
            })
            .filter(|config| {
                self.edition_list
                    .as_ref()
                    .and_then(|list| list.selected())
                    .is_none_or(|edition| Some(edition) == config.edition.as_ref())
            })
            .map(|config| config.release.to_string())
            .unique();
        self.release_list.set_values(releases);
    }

    fn refresh_editions(&mut self) {
        let editions = self
            .selected_os
            .releases
            .iter()
            .filter(|config| {
                self.arch_list
                    .selected()
                    .is_none_or(|arch| arch == &config.arch)
            })
            .filter(|config| {
                self.release_list
                    .selected()
                    .is_none_or(|release| release == &config.release)
            })
            .filter_map(|config| config.edition.as_deref().map(ToString::to_string))
            .unique();
        match self.edition_list {
            Some(ref mut list) => list.set_values(editions),
            None => self.edition_list = Some(SelectableComboBox::new(editions, None)),
        }
    }

    fn refresh_architectures(&mut self) {
        let architectures = self
            .selected_os
            .releases
            .iter()
            .filter(|config| {
                self.release_list
                    .selected()
                    .is_none_or(|release| release == &config.release)
            })
            .filter(|config| {
                self.edition_list
                    .as_ref()
                    .and_then(|list| list.selected())
                    .is_none_or(|edition| Some(edition) == config.edition.as_ref())
            })
            .map(|config| config.arch)
            .unique_by(|a| a.to_string());
        self.arch_list.set_values(architectures);
    }

    fn view(&self) -> Element<crate::app::Message> {
        let mut list = widget::list_column();
        let row = {
            let mut row = widget::row();

            let release_dropdown = self.release_list.widget("Release", |release| {
                Message::SelectedRelease(release).into()
            });
            row = row.push(release_dropdown);

            let edition_dropdown = self.edition_list.as_ref().map(|edition_list| {
                edition_list.widget("Edition", |edition| {
                    Message::SelectedEdition(edition).into()
                })
            });
            row = row.push_maybe(edition_dropdown);

            let arch_dropdown = self
                .arch_list
                .widget("Architecture", |arch| Message::SelectedArch(arch).into());
            row.push(arch_dropdown)
        };

        list = list.add(row);

        list.into()
    }
}

#[derive(Clone, Debug, Default)]
pub enum Page {
    #[default]
    Loading,
    SelectOS,
    Options,
    Downloading,
    Docker,
    Complete,
    Error(String),
}

#[derive(Clone, Debug)]
pub enum Message {
    OSList(Result<Vec<OS>, String>),
    SelectedOS(OS),
    SelectedRelease(String),
    SelectedEdition(String),
    SelectedArch(Arch),
}

impl From<Message> for crate::app::Message {
    fn from(value: Message) -> Self {
        crate::app::Message::Creation(value)
    }
}
