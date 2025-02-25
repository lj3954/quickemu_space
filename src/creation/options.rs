// SPDX-License-Identifier: GPL-3.0-only

use std::{path::PathBuf, sync::LazyLock};

use ashpd::desktop::file_chooser::SelectedFiles;
use cosmic::{
    app::command::Task,
    iced::{Alignment, Length},
    widget::{self, icon},
    Element,
};
use itertools::Itertools;
use quickemu_core::data::{AArch64Machine, Arch, Riscv64Machine, X86_64Machine};
use quickget_core::{data_structures::OS, QuickgetConfig, QuickgetInstance};

use super::{Page, SelectableComboBox};

static TOTAL_CPU_CORES: LazyLock<f64> =
    LazyLock::new(|| QuickgetInstance::get_total_cpu_cores() as f64);
static RECOMMENDED_CPU_CORES: LazyLock<usize> =
    LazyLock::new(QuickgetInstance::get_recommended_cpu_cores);
static TOTAL_RAM: LazyLock<f64> = LazyLock::new(|| QuickgetInstance::get_total_ram() as f64);
static RECOMMENDED_RAM: LazyLock<f64> =
    LazyLock::new(|| QuickgetInstance::get_recommended_ram() as f64);

#[derive(Debug, Clone)]
pub(crate) struct OptionSelection {
    selected_os: OS,
    release_list: SelectableComboBox<String>,
    edition_list: SelectableComboBox<String>,
    arch_list: SelectableComboBox<Arch>,
    cpu_cores: usize,
    ram: f64,
    vm_name: Option<String>,
    default_vm_name: Option<String>,
    directory: PathBuf,
}

impl OptionSelection {
    pub(super) fn new(selected_os: OS, default_vm_dir: PathBuf) -> Self {
        let mut options = Self {
            selected_os,
            release_list: SelectableComboBox::new_empty(),
            edition_list: SelectableComboBox::new_empty(),
            arch_list: SelectableComboBox::new_empty(),
            cpu_cores: *RECOMMENDED_CPU_CORES,
            ram: *RECOMMENDED_RAM,
            directory: default_vm_dir,
            vm_name: None,
            default_vm_name: None,
        };
        options.refresh_releases();
        options.refresh_editions();
        options.refresh_architectures();

        let preferred_arch = match std::env::consts::ARCH {
            "aarch64" => Arch::AArch64 {
                machine: AArch64Machine::Standard,
            },
            "riscv64" => Arch::Riscv64 {
                machine: Riscv64Machine::Standard,
            },
            _ => Arch::X86_64 {
                machine: X86_64Machine::Standard,
            },
        };
        options.arch_list.try_select(preferred_arch);

        options
    }

    pub(super) fn to_instance(&self, vm_name: &str) -> Result<QuickgetInstance, String> {
        let qg_config = QuickgetConfig {
            os: self.selected_os.name.clone(),
            config: self
                .selected_os
                .releases
                .iter()
                .filter(|config| self.release_list.selected().unwrap() == &config.release)
                .filter(|config| self.edition_list.selected() == config.edition.as_ref())
                .find(|config| self.arch_list.selected().unwrap() == &config.arch)
                .cloned()
                .expect("A config should be present"),
        };
        QuickgetInstance::new_with_vm_name(qg_config, self.directory.clone(), vm_name)
            .map_err(|e| e.to_string())
    }

    pub(super) fn update(&mut self, msg: Message) -> Task<crate::app::Message> {
        match msg {
            Message::SelectedRelease(release) => self.select_release(release),
            Message::SelectedEdition(edition) => self.select_edition(edition),
            Message::SelectedArch(arch) => self.select_arch(arch),
            Message::SetRAM(ram) => self.ram = ram,
            Message::SetCPUCores(cores) => self.cpu_cores = cores,
            Message::SelectVMDir => {
                return Task::perform(crate::app::select_dir(), |dir| {
                    match dir {
                        Some(dir) => crate::app::Message::from(Message::SelectedVMDir(dir)),
                        _ => crate::app::Message::None,
                    }
                    .into()
                })
            }
            Message::SelectedVMDir(dir) => self.directory = dir,
            Message::SelectedVMName(name) => self.vm_name = Some(name),
            Message::FinalizeVMName => {
                if self
                    .vm_name
                    .as_ref()
                    .is_some_and(|vm_name| vm_name.is_empty())
                {
                    self.vm_name = None;
                }
            }
        }
        Task::none()
    }

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
                    .selected()
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

        self.edition_list.set_values(editions);
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
                    .selected()
                    .is_none_or(|edition| Some(edition) == config.edition.as_ref())
            })
            .map(|config| config.arch)
            .unique_by(|a| a.to_string());
        self.arch_list.set_values(architectures);
    }

    fn select_release(&mut self, release: String) {
        self.release_list.select(Some(release));
        self.refresh_editions();
        self.refresh_architectures();
        self.set_default_vm_name();
    }

    fn select_edition(&mut self, edition: String) {
        self.edition_list.select(Some(edition));
        self.refresh_releases();
        self.refresh_architectures();
        self.set_default_vm_name();
    }

    fn select_arch(&mut self, arch: Arch) {
        self.arch_list.select(Some(arch));
        self.refresh_releases();
        self.refresh_editions();
        self.set_default_vm_name();
    }

    fn set_default_vm_name(&mut self) {
        self.default_vm_name = match (
            self.release_list.selected(),
            self.edition_list.selected().map(|e| e.as_str()),
            self.edition_list.is_empty(),
            self.arch_list.selected(),
        ) {
            (Some(release), edition, no_editions, Some(arch))
                if edition.is_some() || no_editions =>
            {
                Some(default_vm_name(&self.selected_os, release, edition, *arch))
            }
            _ => None,
        };
    }

    pub(super) fn view(&self) -> Element<crate::app::Message> {
        let mut list = widget::list_column();

        let vm_name = self.vm_name.as_deref().or(self.default_vm_name.as_deref());
        let vm_name_row = {
            let vm_name_text = widget::text("VM Name:  ");

            let displayed_vm_name = vm_name.unwrap_or_default();
            let vm_name_input = widget::text_input("VM Name", displayed_vm_name)
                .on_input(|name| Message::SelectedVMName(name).into())
                .on_submit(Message::FinalizeVMName.into());

            widget::row()
                .align_y(Alignment::Center)
                .push(vm_name_text)
                .push(vm_name_input)
        };
        list = list.add(vm_name_row);

        let os_row = {
            let mut row = widget::row();

            let release_dropdown = self.release_list.widget("Release", |release| {
                Message::SelectedRelease(release).into()
            });
            row = row.push_maybe(release_dropdown);

            let edition_dropdown = self.edition_list.widget("Edition", |edition| {
                Message::SelectedEdition(edition).into()
            });
            row = row.push_maybe(edition_dropdown);

            let arch_dropdown = self
                .arch_list
                .widget("Architecture", |arch| Message::SelectedArch(arch).into());
            row.push_maybe(arch_dropdown)
        };
        list = list.add(os_row);

        let cpu_row = {
            let cpu_text = widget::text("CPU Cores:  ");
            let cpu_slider = widget::slider(1.0..=*TOTAL_CPU_CORES, self.cpu_cores as f64, |x| {
                Message::SetCPUCores(x as usize).into()
            });
            let selected_cpu_text = widget::text(format!("  {}", self.cpu_cores));
            widget::row()
                .align_y(Alignment::Center)
                .push(cpu_text)
                .push(cpu_slider)
                .push(selected_cpu_text)
        };
        list = list.add(cpu_row);

        let ram_row = {
            let ram_text = widget::text("RAM:  ");
            let ram_slider = widget::slider(
                100.0 * size::consts::MiB as f64..=*TOTAL_RAM,
                self.ram,
                |x| Message::SetRAM(x).into(),
            )
            .step(0.0001);
            let selected_ram_text = widget::text(format!("  {}", size::Size::from_bytes(self.ram)));

            widget::row()
                .align_y(Alignment::Center)
                .push(ram_text)
                .push(ram_slider)
                .push(selected_ram_text)
        };
        list = list.add(ram_row);

        let dir_row = {
            let dir_text = widget::text("VM Directory:  ");
            let dir_input =
                widget::text_input("VM Directory", self.directory.display().to_string())
                    .on_input(|dir| Message::SelectedVMDir(PathBuf::from(dir)).into());
            let dir_open_button = widget::button::icon(icon::from_name("folder-open-symbolic"))
                .on_press(Message::SelectVMDir.into())
                .tooltip("Select VM Directory");

            widget::row()
                .align_y(Alignment::Center)
                .push(dir_text)
                .push(dir_input)
                .push(dir_open_button)
        };
        list = list.add(dir_row);

        list = list.add(widget::vertical_space());

        let nav_row = {
            let mut row = widget::row();

            let back = widget::button::suggested("Back")
                .on_press(super::Message::ChangePage(Page::SelectOS.into()).into());
            row = row.push(back);

            let next = widget::button::suggested("Next");
            let next = match vm_name {
                Some(vm_name) if self.can_go_next(vm_name) => {
                    next.on_press(super::Message::StartDownloads(vm_name.to_owned()).into())
                }
                _ => next,
            };

            row.push(
                widget::container(next)
                    .align_right(Length::Shrink)
                    .width(Length::Fill),
            )
        };
        list = list.add(nav_row);

        list.into()
    }

    fn can_go_next(&self, vm_name: &str) -> bool {
        !vm_name.is_empty()
            && !vm_name.contains('/')
            && self.default_vm_name.is_some()
            && self.directory.exists()
            && !self.directory.join(vm_name).exists()
    }
}

fn default_vm_name(os: &OS, release: &str, edition: Option<&str>, arch: Arch) -> String {
    let mut vm_name = format!("{}-{}", os.name, release);
    if let Some(edition) = edition {
        vm_name.push('-');
        vm_name.push_str(edition)
    }

    let snake_case_arch: String = arch
        .to_string()
        .chars()
        .map(|c| {
            if c == ' ' {
                '_'
            } else {
                c.to_ascii_lowercase()
            }
        })
        .filter(|c| c.is_alphanumeric() || *c == '_')
        .collect();
    vm_name.push('-');
    vm_name.push_str(&snake_case_arch);

    vm_name
}

#[derive(Clone, Debug)]
pub(crate) enum Message {
    SelectedRelease(String),
    SelectedEdition(String),
    SelectedArch(Arch),
    SetRAM(f64),
    SetCPUCores(usize),
    SelectVMDir,
    SelectedVMDir(PathBuf),
    SelectedVMName(String),
    FinalizeVMName,
}

impl From<Message> for crate::app::Message {
    fn from(value: Message) -> Self {
        crate::app::Message::Creation(super::Message::Options(value))
    }
}
