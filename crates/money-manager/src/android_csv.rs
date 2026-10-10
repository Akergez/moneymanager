//! Save CSV through Android's document provider, on the book's worker thread.

use gpui_mobile::android::jni as bridge;
use jni::objects::JValue;

/// The system picker grants write access to the selected document. A cancelled
/// picker returns false; no persistent URI or broad storage access is retained.
pub(crate) fn export(bytes: &[u8]) -> Result<bool, String> {
    bridge::with_env(|env| {
        let activity = bridge::activity(env)?;
        let class = bridge::find_app_class(env, "dev.gpui.mobile.GpuiPathPicker")?;
        let bytes = env
            .byte_array_from_slice(bytes)
            .map_err(|e| e.to_string())?;
        env.call_static_method(
            &class,
            jni::jni_str!("saveCsv"),
            jni::jni_sig!("(Landroid/app/Activity;[B)Z"),
            &[JValue::Object(&activity), JValue::Object(bytes.as_ref())],
        )
        .and_then(|value| value.z())
        .map_err(|error| {
            env.exception_clear();
            format!("Could not save CSV: {error}")
        })
    })
}
