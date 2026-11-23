package app.akergez.moneymanager;

import android.view.InputDevice;
import android.view.KeyEvent;
import dev.gpui.mobile.GpuiActivity;

public final class MoneyManagerActivity extends GpuiActivity {
    @Override
    public boolean dispatchKeyEvent(KeyEvent event) {
        return dispatchPhysicalKey(event) || super.dispatchKeyEvent(event);
    }

    @Override
    public boolean dispatchKeyShortcutEvent(KeyEvent event) {
        return dispatchPhysicalKey(event) || super.dispatchKeyShortcutEvent(event);
    }

    private boolean dispatchPhysicalKey(KeyEvent event) {
        int action = event.getAction();
        if (!event.isFromSource(InputDevice.SOURCE_KEYBOARD)
                || (event.getFlags() & KeyEvent.FLAG_SOFT_KEYBOARD) != 0
                || event.isSystem()
                || (action != KeyEvent.ACTION_DOWN && action != KeyEvent.ACTION_UP)) {
            return false;
        }
        // Use the actual device's key character map before the input proxy
        // turns the event into an IME edit and drops its modifiers.
        int character = event.isCtrlPressed() || event.isMetaPressed() ? 0 : event.getUnicodeChar();
        return nativePhysicalKey(event.getKeyCode(), event.getMetaState(), character,
                action == KeyEvent.ACTION_DOWN, event.getRepeatCount() != 0);
    }

    private static native boolean nativePhysicalKey(int code, int modifiers, int character,
            boolean down, boolean repeat);
}
