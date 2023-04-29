use std::time::Duration;

use crate::app::task_queue::{PollResult, PollingData, SleepTask, TaskQueue};

// fn prog_check(prog: f32) -> bool {
//     prog > 0.0 && prog < 1.0
// }

// #[derive(serde::Deserialize, serde::Serialize)]
// #[serde(default)]
pub struct TemplateApp {
    label: String,

    // #[serde(skip)]
    task_queue: TaskQueue,
    task_ids: Vec<usize>,
    // #[serde(skip)]
    value: f32,
}

impl Default for TemplateApp {
    fn default() -> Self {
        Self {
            label: "Task Queue UI".to_owned(),
            task_queue: TaskQueue::new(),
            task_ids: Vec::new(),
            value: 1.0,
        }
    }
}

impl TemplateApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        // if let Some(storage) = cc.storage {
        //     return eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default();
        // }

        Default::default()
    }
}

impl eframe::App for TemplateApp {
    // fn save(&mut self, storage: &mut dyn eframe::Storage) {
    //     eframe::set_value(storage, eframe::APP_KEY, self);
    // }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::Window::new("Controls Window")
            .constrain(true)
            .fixed_size((
                _frame.info().window_info.size.x - 16.0,
                _frame.info().window_info.size.y - 420.0,
            ))
            .show(ctx, |ui| {
                ui.heading("Controls");
                // TODO: add content type select box
                // fetch content type task from list of demo resources / apis
                ui.horizontal(|ui| {
                    ui.label("Select a content type: ");
                    ui.text_edit_singleline(&mut self.label.to_string());
                });

                ui.separator();

                ui.group(|ui| {
                    ui.add(egui::Slider::new(&mut self.value, 1.0..=10.0).text("value"));
                    if ui.button("Increment").clicked() {
                        self.value += 1.0;
                    }

                    if ui.button("Add task").clicked() {
                        let task = SleepTask::new(0, Duration::from_secs(self.value.ceil() as u64));
                        let task_id = self.task_queue.add_task(task);
                        self.task_ids.push(task_id);
                    }
                });

                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
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

        egui::Window::new(format!(
            "Currently tracking {} tasks...",
            self.task_ids.len()
        ))
        .id(egui::Id::new("main_window"))
        .constrain(true)
        .default_pos((0.0, _frame.info().window_info.size.y - 420.0))
        .default_size((
            _frame.info().window_info.size.x - 16.0,
            _frame.info().window_info.size.y - 80.0,
        ))
        .show(ctx, |ui| {
            // ui.heading(format!("Currently tracking {} tasks...", self.task_ids.len()));
            // Main window content goes here
            if !self.task_ids.is_empty() {
                self.task_ids
                    .retain(|task_id| match self.task_queue.poll_task(*task_id) {
                        Ok(PollResult::Completed) => {
                            log::info!("Task {} completed", task_id);
                            false
                        }
                        Ok(PollResult::Cancelled) => {
                            log::info!("Task {} cancelled", task_id);
                            false
                        }
                        _ => true,
                    });

                for task_id in &mut self.task_ids {
                    if let Ok(poll_result) = self.task_queue.poll_task(*task_id) {
                        match poll_result {
                            PollResult::Pending(progress) => match progress {
                                PollingData::Float(p) => {
                                    log::info!("Task {} progress: {}", task_id, p);
                                    ui.group(|ui| {
                                        ui.label(format!("Task {}", task_id));
                                        ui.add(
                                            egui::ProgressBar::new(p)
                                                .desired_width(
                                                    _frame.info().window_info.size.x - 24.0,
                                                )
                                                .fill(egui::Color32::DARK_GREEN),
                                        );
                                        if ui.button("Cancel").clicked() {
                                            if let Err(r) = self.task_queue._remove_task(*task_id) {
                                                log::error!(
                                                    "Task {} cancellation error: {:?}",
                                                    task_id,
                                                    r
                                                );
                                            } else {
                                                log::info!("Task {} cancelled", task_id);
                                            }
                                        }
                                    });
                                } // _ => {
                                  //     log::debug!("Task progress: {:?}", progress)
                                  // }
                            },
                            PollResult::Completed => {
                                log::info!("Task {} completed", task_id);
                                drop(*task_id)
                            }
                            PollResult::Cancelled => {
                                log::info!("Task {} cancelled", task_id);
                            }
                            PollResult::Paused => {
                                log::info!("Task {} paused", task_id);
                            }
                        }
                    }
                }

                if ui.button("Cancel all tasks").clicked() {
                    for task_id in &self.task_ids {
                        if let Err(r) = self.task_queue._remove_task(*task_id) {
                            log::error!("Task {} cancellation error: {:?}", task_id, r);
                        } else {
                            log::info!("Task {} cancelled", task_id);
                        };
                    }
                    self.task_ids.clear();
                }
            }

            egui::warn_if_debug_build(ui);
        });
        ctx.request_repaint_after(Duration::from_millis(16));
    }
}
