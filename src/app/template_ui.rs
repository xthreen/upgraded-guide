use std::time::Duration;

use crate::app::task_queue::{TaskQueue, SleepTask, PollingData, PollResult};

fn prog_check(prog: f32) -> bool {
    prog > 0.0 && prog < 1.0
}

/// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
pub struct TemplateApp {
    // Example stuff:
    label: String,

    // this how you opt-out of serialization of a member
    #[serde(skip)]
    task_queue: TaskQueue,
    #[serde(skip)]
    cur_task: Option<usize>,
    #[serde(skip)]
    task_progress: f32,
    #[serde(skip)]
    poll_result: Option<PollResult>,
    #[serde(skip)]
    value: f32,
}

impl Default for TemplateApp {
    fn default() -> Self {
        Self {
            // Example stuff:
            label: "Task Queue UI".to_owned(),
            task_queue: TaskQueue::new(),
            task_progress: 0.0,
            cur_task: None,
            poll_result: None,
            value: 1.0,
        }
    }
}

impl TemplateApp {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // This is also where you can customize the look and feel of egui using
        // `cc.egui_ctx.set_visuals` and `cc.egui_ctx.set_fonts`.

        // Load previous app state (if any).
        // Note that you must enable the `persistence` feature for this to work.
        if let Some(storage) = cc.storage {
            return eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default();
        }

        Default::default()
    }
}

impl eframe::App for TemplateApp {
    /// Called by the frame work to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    /// Called each time the UI needs repainting, which may be many times per second.
    /// Put your widgets into a `SidePanel`, `TopPanel`, `CentralPanel`, `Window` or `Area`.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Examples of how to create different panels and windows.
        // Pick whichever suits you.
        // Tip: a good default choice is to just keep the `CentralPanel`.
        // For inspiration and more examples, go to https://emilk.github.io/egui

        #[cfg(not(target_arch = "wasm32"))] // no File->Quit on web pages!
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            // The top panel is often a good place for a menu bar:
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Quit").clicked() {
                        _frame.close();
                    }
                });
            });
        });

        egui::SidePanel::left("side_panel").show(ctx, |ui| {
            ui.heading("Side Panel");

            ui.horizontal(|ui| {
                ui.label("Write something: ");
                ui.text_edit_singleline(&mut self.label.to_string());
            });

            ui.add(egui::Slider::new(&mut self.value, 1.0..=10.0).text("value"));
            if ui.button("Increment").clicked() {
                self.value += 1.0;
            }

            if ui.button("Add task").clicked() {
                let task = SleepTask::new(0, Duration::from_secs(self.value.ceil() as u64));
                let task_id = self.task_queue.add_task(task);
                self.cur_task = Some(task_id);
            }

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

        egui::CentralPanel::default().show(ctx, |ui| {
            // The central panel the region left after adding TopPanel's and SidePanel's
            let progress_bar = egui::ProgressBar::new(self.task_progress).desired_width(_frame.info().window_info.size.x - 256.0).fill(egui::Color32::LIGHT_GREEN);
            ui.add_visible_ui(prog_check(self.task_progress), |ui| {
                ui.add(progress_bar);
            });
            if let Some(cur_task) = self.cur_task {
                if let Ok(poll_result) = self.task_queue.poll_task(cur_task) {
                    match poll_result {
                        PollResult::Pending(progress) => {
                            match progress {
                                PollingData::Float(p) => {
                                    log::info!("Task progress: {}", p);
                                    self.task_progress = p;
                                }
                                _ => {
                                    log::debug!("Task progress: {:?}", progress)
                                }
                            }
                        }
                        PollResult::Completed => {
                            log::info!("Task completed");
                            self.task_progress = 0.0;
                            self.cur_task = None;
                        }
                        PollResult::Cancelled => {
                            log::info!("Task cancelled");
                            self.task_progress = 0.0;
                            self.cur_task = None;
                        }
                        PollResult::Paused => {
                            log::info!("Task paused... somehow?");
                        }
                    }
                }
            }
            ui.heading("eframe template");
            ui.hyperlink("https://github.com/emilk/eframe_template");
            ui.add(egui::github_link_file!(
                "https://github.com/emilk/eframe_template/blob/master/",
                "Source code."
            ));
            egui::warn_if_debug_build(ui);
        });

        if false {
            egui::Window::new("Window").show(ctx, |ui| {
                ui.label("Windows can be moved by dragging them.");
                ui.label("They are automatically sized based on contents.");
                ui.label("You can turn on resizing and scrolling if you like.");
                ui.label("You would normally choose either panels OR windows.");
            });
        }
        ctx.request_repaint_after(Duration::from_millis(16));
    }
}
