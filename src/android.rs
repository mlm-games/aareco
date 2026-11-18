#![cfg(target_os = "android")]

use jni::{JNIEnv, JavaVM, objects::JObject, objects::JValue, sys::jobject};
use winit::platform::android::activity::AndroidApp;

pub fn install(context: &AndroidApp) {
    if let Some(path) = context.internal_data_path() {
        crate::paths::set_android_data_root(path.to_path_buf());
    }
    rlobkit_dialogs::init_shared_pending_state();
    rlobkit_dialogs::init_with_android_context(
        context.vm_as_ptr().cast(),
        context.activity_as_ptr().cast(),
    );
    rlobkit_app_events::insets::set_on_insets(|insets| {
        repose_core::locals::set_window_insets_default(repose_core::locals::WindowInsets {
            top: insets.top,
            bottom: insets.bottom,
            left: insets.left,
            right: insets.right,
            ime_bottom: insets.ime_bottom,
        });
        repose_platform::wake_event_loop();
    });
    rlobkit_app_events::theme::set_on_theme(|_| repose_platform::wake_event_loop());
}

pub fn request_microphone_permission(context: &AndroidApp) {
    let vm = unsafe { JavaVM::from_raw(context.vm_as_ptr().cast()) };
    let activity = context.activity_as_ptr().cast::<jobject>();
    let result = vm.attach_current_thread(|env| -> jni::errors::Result<()> {
        let activity = unsafe { JObject::from_raw(env, activity) };
        request_permission(env, &activity)
    });
    if let Err(err) = result {
        log::warn!("could not request microphone permission: {err}");
    }
}

fn request_permission(env: &mut JNIEnv, activity: &JObject) -> jni::errors::Result<()> {
    let permission = env.new_string("android.permission.RECORD_AUDIO")?;
    let status = env
        .call_method(
            activity,
            "checkSelfPermission",
            "(Ljava/lang/String;)I",
            &[JValue::Object(&permission)],
        )?
        .i()?;
    if status == 0 {
        return Ok(());
    }
    let array = env.new_object_array(1, "java/lang/String", &permission)?;
    let array = JObject::from(array);
    env.call_method(
        activity,
        "requestPermissions",
        "([Ljava/lang/String;I)V",
        &[JValue::Object(&array), JValue::Int(9102)],
    )?;
    Ok(())
}
