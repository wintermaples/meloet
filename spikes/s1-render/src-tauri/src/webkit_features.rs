// WebKitGTK >= 2.42 feature-flag API. The webkit2gtk 2.0.2 crate only binds up to 2.40, so declare it here.
use std::ffi::{CStr, c_char, c_int, c_void};

use glib::translate::ToGlibPtr;
use webkit2gtk::WebViewExt;

#[link(name = "webkit2gtk-4.1")]
unsafe extern "C" {
    fn webkit_settings_get_all_features() -> *mut c_void;
    fn webkit_feature_list_get_length(list: *mut c_void) -> usize;
    fn webkit_feature_list_get(list: *mut c_void, index: usize) -> *mut c_void;
    fn webkit_feature_list_unref(list: *mut c_void);
    fn webkit_feature_get_identifier(feature: *mut c_void) -> *const c_char;
    fn webkit_settings_set_feature_enabled(settings: *mut c_void, feature: *mut c_void, enabled: c_int);
    fn webkit_settings_get_feature_enabled(settings: *mut c_void, feature: *mut c_void) -> c_int;
}

/// Sets a feature by identifier. Returns the value before the change, or None when the id is unknown.
pub fn set(view: &webkit2gtk::WebView, id: &str, enabled: bool) -> Option<bool> {
    let settings = view.settings()?;
    let sp: *mut webkit2gtk::ffi::WebKitSettings = settings.to_glib_none().0;
    unsafe {
        let list = webkit_settings_get_all_features();
        let mut before = None;
        for i in 0..webkit_feature_list_get_length(list) {
            let f = webkit_feature_list_get(list, i);
            if CStr::from_ptr(webkit_feature_get_identifier(f)).to_bytes() == id.as_bytes() {
                before = Some(webkit_settings_get_feature_enabled(sp.cast(), f) != 0);
                webkit_settings_set_feature_enabled(sp.cast(), f, enabled as c_int);
            }
        }
        webkit_feature_list_unref(list);
        before
    }
}

pub fn dump(view: &webkit2gtk::WebView) {
    let Some(settings) = view.settings() else { return };
    let sp: *mut webkit2gtk::ffi::WebKitSettings = settings.to_glib_none().0;
    unsafe {
        let list = webkit_settings_get_all_features();
        for i in 0..webkit_feature_list_get_length(list) {
            let f = webkit_feature_list_get(list, i);
            let id = CStr::from_ptr(webkit_feature_get_identifier(f)).to_string_lossy();
            eprintln!("feature {} = {}", id, webkit_settings_get_feature_enabled(sp.cast(), f));
        }
        webkit_feature_list_unref(list);
    }
}
