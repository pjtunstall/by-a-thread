async fn handle(ui: &mut dyn crate::lobby::ui::LobbyUi) {
    loop {
        ui.draw(false, false);
        if is_key_pressed(KeyCode::Escape) || is_quit_requested() {
            break;
        }

        next_frame().await;
    }
}
