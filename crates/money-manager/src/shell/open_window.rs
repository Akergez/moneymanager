use gpui_kit::component::TitleBar;
use gpui_kit::{App, AppContext, Bounds, WindowBounds, WindowOptions};

use super::Session;
use super::shell_view::Shell;
use super::window_size::{initial_size, minimum_size};

pub fn open_window(cx: &mut App) {
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
            None,
            initial_size(cx),
            cx,
        ))),
        window_min_size: Some(minimum_size(cx)),
        app_id: Some(crate::APP_ID.to_string()),
        ..TitleBar::window_options()
    };
    match gpui_kit::open_window(options, cx, |window, cx| {
        cx.new(|cx| Shell::new(window, cx))
    }) {
        Ok((handle, shell)) => {
            cx.global_mut::<Session>().window = Some((handle, shell.clone()));
            let _ = handle.update(cx, |_, window, cx| {
                window.set_window_title("Money Manager");
                shell.update(cx, |shell, cx| shell.start(window, cx));
            });
            crate::script::play(handle, cx);
        }
        Err(error) => tracing::error!(%error, "could not open a window"),
    }
}
