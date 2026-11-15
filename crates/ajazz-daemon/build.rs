#[path = "../../tools/embed_win_icon.rs"]
mod embed_win_icon;

fn main() {
    embed_win_icon::embed_ico(std::path::Path::new("../ajazz-gui/ui/assets/logo.ico"));
}
