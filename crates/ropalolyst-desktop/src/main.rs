use eframe::egui;

fn main() -> eframe::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1100.0, 720.0])
            .with_min_inner_size([820.0, 560.0])
            .with_title("Ropalolyst Analytics"),
        ..Default::default()
    };

    eframe::run_native(
        "Ropalolyst Analytics",
        options,
        Box::new(|_cc| Ok(Box::new(RopalolystApp::default()))),
    )
}

#[derive(Default)]
struct RopalolystApp {
    selected_tab: Tab,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum Tab {
    #[default]
    Live,
    History,
    Settings,
}

impl eframe::App for RopalolystApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("header").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Ropalolyst Analytics");
                ui.separator();
                ui.label("Warframe · SolNode740");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label("Prototype");
                });
            });
        });

        egui::SidePanel::left("navigation")
            .resizable(false)
            .default_width(170.0)
            .show(ctx, |ui| {
                ui.add_space(8.0);
                ui.selectable_value(&mut self.selected_tab, Tab::Live, "Live analysis");
                ui.selectable_value(&mut self.selected_tab, Tab::History, "Run history");
                ui.selectable_value(&mut self.selected_tab, Tab::Settings, "Settings");
            });

        egui::CentralPanel::default().show(ctx, |ui| match self.selected_tab {
            Tab::Live => {
                ui.heading("Live analysis");
                ui.label("Waiting for a Warframe EE.log source.");
                ui.add_space(16.0);
                ui.horizontal_wrapped(|ui| {
                    metric(ui, "MISSION", "—");
                    metric(ui, "ELAPSED", "00:00.000");
                    metric(ui, "BOSS PHASE", "Not detected");
                });
                ui.separator();
                ui.heading("Encounter timeline");
                ui.label("Verified events will appear here when the log parser is connected.");
            }
            Tab::History => {
                ui.heading("Run history");
                ui.label("No runs recorded yet.");
            }
            Tab::Settings => {
                ui.heading("Settings");
                ui.label("EE.log path selection and privacy controls will be implemented here.");
            }
        });
    }
}

fn metric(ui: &mut egui::Ui, label: &str, value: &str) {
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.set_min_width(190.0);
        ui.label(egui::RichText::new(label).small().weak());
        ui.heading(value);
    });
}
