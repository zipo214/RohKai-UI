// First manually verified RohKai output.
// This is the code the designer should generate for a single Button widget.

fn main() -> eframe::Result<()> {
    eframe::run_native(
        "Hello Button",
        eframe::NativeOptions::default(),
        Box::new(|_cc| Ok(Box::new(HelloApp::default()))),
    )
}

#[derive(Default)]
struct HelloApp;

impl eframe::App for HelloApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            if ui.button("Hello").clicked() {
                println!("clicked!");
            }
        });
    }
}
