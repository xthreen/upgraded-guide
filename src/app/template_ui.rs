use std::time::Duration;

use crate::app::sleep_task::SleepTask;
use crate::app::task_queue::{PollResult, PollingData, TaskQueue};

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct TemplateApp {
    label: String,
    show_footer: bool,
    show_header: bool,
    #[serde(skip)]
    task_queue: TaskQueue,
    #[serde(skip)]
    task_ids: Vec<usize>,
    #[serde(skip)]
    value: f32,
}

impl Default for TemplateApp {
    fn default() -> Self {
        Self {
            label: "Task Queue UI".to_owned(),
            show_footer: false,
            show_header: true,
            task_queue: TaskQueue::new(),
            task_ids: Vec::new(),
            value: 1.0,
        }
    }
}

impl TemplateApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        if let Some(storage) = cc.storage {
            return eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default();
        }

        Default::default()
    }

    fn ui_menubar(&mut self, ui: &mut egui::Ui) {
        egui::menu::bar(ui, |ui| {
            ui.separator();
            ui.menu_button("Options", |ui| {
                ui.checkbox(&mut self.show_header, "Show header");
                ui.checkbox(&mut self.show_footer, "Show footer");
            });
            ui.separator();
        });
    }
}

impl eframe::App for TemplateApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("header_panel").show_animated(ctx, self.show_header, |ui| {
            TemplateApp::ui_menubar(self, ui);
            ui.separator();
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                ui.label("Task Queue UI");
            });
        });
        egui::TopBottomPanel::bottom("footer_panel").show_animated(ctx, self.show_footer, |ui| {
            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                egui::warn_if_debug_build(ui);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    ui.label("powered by ");
                    ui.hyperlink_to("egui", "https://github.com/emilk/egui");
                    ui.label(" and ");
                    ui.hyperlink_to(
                        "eframe",
                        "https://github.com/emilk/egui/tree/master/crates/eframe",
                    );
                    ui.label(".");
                });
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            if !self.show_header {
                TemplateApp::ui_menubar(self, ui);
            }
            ui.separator();
            ui.heading("Controls");
            ui.group(|ui| {
                ui.add(egui::Slider::new(&mut self.value, 1.0..=10.0).text("value"));
                if ui.button("Increment").clicked() {
                    self.value += 1.0;
                }
                if ui.button("Add task").clicked() {
                    let task = SleepTask::new(None, Duration::from_secs(self.value.ceil() as u64));
                    let task_id = self.task_queue.add_task(task);
                    self.task_ids.push(task_id);
                }
            });
            ui.separator();

            ui.heading(format!(
                "Currently tracking {} tasks...",
                self.task_ids.len()
            ));
            ui.separator();

            egui::ScrollArea::vertical()
                .drag_to_scroll(true)
                .max_height(_frame.info().window_info.size.y - 100.0)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    if !self.task_ids.is_empty() {
                        self.task_ids
                            .retain(|task_id| match self.task_queue.poll_task(*task_id) {
                                Ok(PollResult::Completed) => {
                                    log::debug!("Task {} completed, filtering", task_id);
                                    false
                                }
                                Ok(PollResult::Cancelled) => {
                                    log::debug!("Task {} cancelled", task_id);
                                    false
                                }
                                _ => true,
                            });
                        if ui.button("Cancel all tasks").clicked() {
                            for task_id in &self.task_ids {
                                if let Err(r) = self.task_queue.remove_task(*task_id) {
                                    log::error!("Task {} cancellation error: {:?}", task_id, r);
                                } else {
                                    log::debug!("Task {} cancelled", task_id);
                                };
                            }
                            self.task_ids.clear();
                        }
                        for task_id in &mut self.task_ids {
                            if let Ok(poll_result) = self.task_queue.poll_task(*task_id) {
                                match poll_result {
                                    PollResult::Pending(progress) => match progress {
                                        PollingData::Float(p) => {
                                            log::debug!("Task {} progress: {}", task_id, p);
                                            ui.group(|ui| {
                                                ui.label(format!("Task {}", task_id));
                                                ui.add(
                                                    egui::ProgressBar::new(p)
                                                        .desired_width(
                                                            _frame.info().window_info.size.x - 36.0,
                                                        )
                                                        .fill(egui::Color32::DARK_GREEN),
                                                );
                                                if ui.button("Pause").clicked() {
                                                    if let Err(r) =
                                                        self.task_queue.pause_task(*task_id)
                                                    {
                                                        log::error!(
                                                            "Task {} pause error: {:?}",
                                                            task_id,
                                                            r
                                                        );
                                                    } else {
                                                        log::debug!("Task {} paused", task_id);
                                                    }
                                                }
                                                if ui.button("Cancel").clicked() {
                                                    if let Err(r) =
                                                        self.task_queue.remove_task(*task_id)
                                                    {
                                                        log::error!(
                                                            "Task {} cancellation error: {:?}",
                                                            task_id,
                                                            r
                                                        );
                                                    } else {
                                                        log::debug!("Task {} cancelled", task_id);
                                                    }
                                                }
                                            });
                                        }
                                    },
                                    PollResult::Completed => {
                                        log::debug!("Task {} completed", task_id);
                                    }
                                    PollResult::Cancelled => {
                                        log::debug!("Task {} cancelled", task_id);
                                    }
                                    PollResult::Paused(progress) => match progress {
                                        PollingData::Float(p) => {
                                            log::debug!("Task {} paused at {}", task_id, p);
                                            ui.group(|ui| {
                                                ui.label(format!("Task {} paused", task_id));
                                                ui.add(
                                                    egui::ProgressBar::new(p)
                                                        .desired_width(
                                                            _frame.info().window_info.size.x - 36.0,
                                                        )
                                                        .fill(egui::Color32::DARK_GREEN),
                                                );
                                                if ui.button("Resume").clicked() {
                                                    if let Err(r) =
                                                        self.task_queue.resume_task(*task_id)
                                                    {
                                                        log::error!(
                                                            "Task {} resume error: {:?}",
                                                            task_id,
                                                            r
                                                        );
                                                    } else {
                                                        log::debug!("Task {} resumed", task_id);
                                                    }
                                                }
                                                if ui.button("Cancel").clicked() {
                                                    if let Err(r) =
                                                        self.task_queue.remove_task(*task_id)
                                                    {
                                                        log::error!(
                                                            "Task {} cancellation error: {:?}",
                                                            task_id,
                                                            r
                                                        );
                                                    } else {
                                                        log::debug!("Task {} cancelled", task_id);
                                                    }
                                                }
                                            });
                                        }
                                    },
                                }
                            }
                        }
                    }
                });
        });
        ctx.request_repaint_after(Duration::from_millis(16));
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }
}
