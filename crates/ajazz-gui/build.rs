#[path = "../../tools/embed_win_icon.rs"]
mod embed_win_icon;

fn main() {
    embed_win_icon::embed_ico(std::path::Path::new("ui/assets/logo.ico"));
    slint_build::compile("ui/app.slint").expect("Falha ao compilar interface Slint");
}
