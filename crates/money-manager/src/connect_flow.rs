//! The server-connect round trip end to end, through the test harness.
//!
//! What a phone does with a browser in between — `SyncConnect::begin`, the
//! deep link's return, the poller's match, the key generated for a new
//! repository, the onboarding consuming it — driven in one window. Frames
//! are pumped explicitly: while an attempt waits, the screen shows an
//! animating Busy, and `run_until_parked` would never return.
//!
//! This is the regression test for a reentrancy deadlock too: an outcome
//! observer that took the outcome unconditionally re-notified the global it
//! observes, forever (`SyncConnect::take_outcome` reads before writing).

#[cfg(test)]
mod tests {
    use gpui_kit::AppContext as _;
    use gpui_kit::test::TestWindowExt as _;

    fn frame(cx: &mut gpui_kit::TestAppContext, window: gpui_kit::AnyWindowHandle) {
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
        })
        .ok();
    }

    #[gpui_kit::test]
    async fn server_connect_round_trip(cx: &mut gpui_kit::TestAppContext) {
        let _ = std::fs::remove_dir_all("/tmp/money-manager-connect-test");
        unsafe {
            std::env::set_var(
                "XDG_CONFIG_HOME",
                "/tmp/money-manager-connect-test/config",
            );
            std::env::set_var("XDG_DATA_HOME", "/tmp/money-manager-connect-test/data");
        }
        cx.update(|cx| {
            gpui_kit::init(cx);
            gpui_adaptive::init(gpui_adaptive::InputMode::Pointer, cx);
            crate::settings::Settings::install(cx);
            crate::sync_server::SyncConnect::install(cx);
        });
        let onboarding =
            cx.add_window(|window, cx| crate::onboarding::Onboarding::new(window, cx));
        let window = onboarding.into();
        frame(cx, window);
        cx.update_window(window, |_, window, cx| {
            window.click("connect-ledger", cx);
        })
        .ok();
        frame(cx, window);
        assert!(!cx.update(|cx| crate::sync_server::SyncConnect::waiting(cx)));

        // The user pressed "Sign in on the server": the attempt begins and is
        // kept in the settings across restarts.
        let page = cx
            .update(|cx| crate::sync_server::SyncConnect::begin(cx, "https://sync.example.com"))
            .unwrap();
        assert!(page.starts_with("https://sync.example.com/?return_to="));
        assert!(cx.update(|cx| crate::sync_server::SyncConnect::waiting(cx)));
        assert!(
            cx.update(|cx| crate::settings::Settings::global(cx).pending_connect()).is_some()
        );

        // The browser comes back with the repository (a new one: no key).
        let attempt = cx.update(|cx| crate::sync_server::SyncConnect::attempt(cx)).unwrap();
        let config =
            "tresse://http?url=https://sync.example.com&repo_id=money&token=trs_a&new=1";
        let mut encoded = String::new();
        for byte in config.bytes() {
            if byte.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&byte) {
                encoded.push(byte as char);
            } else {
                encoded.push_str(&format!("%{byte:02X}"));
            }
        }
        cx.update(|_| {
            crate::sync_server::push_link_for_test(&format!(
                "moneymanager://tresse-connect?state={}&config={}",
                attempt.state, encoded
            ));
        });

        // The poller matches the link against the attempt and reports it.
        for _ in 0..20 {
            cx.executor().advance_clock(std::time::Duration::from_millis(100));
            frame(cx, window);
            if !cx.update(|cx| crate::sync_server::SyncConnect::waiting(cx)) {
                break;
            }
        }
        assert!(!cx.update(|cx| crate::sync_server::SyncConnect::waiting(cx)));
        assert!(
            cx.update(|cx| crate::settings::Settings::global(cx).pending_connect()).is_none()
        );

        // The onboarding consumed the outcome and started connecting (the
        // sync itself fails: there is no such host).
        let entity = onboarding.root(cx).unwrap();
        let (connecting, step, error) =
            cx.read_entity(&entity, |view: &crate::onboarding::Onboarding, _| {
                (
                    view.is_connecting(),
                    view.step(),
                    view.error().map(|e| e.to_string()),
                )
            });
        assert_eq!(step, crate::onboarding::Step::Connect);
        assert!(
            connecting || error.is_some(),
            "the returned repository should start the first sync or fail it, got neither"
        );
    }
}
