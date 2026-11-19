//! An LV2 plugin's own window, for the one kind meloet opens: `ui:X11UI`, a library that
//! makes an X11 window inside the one it is given (`ui:parent`).
//!
//! The library is loaded here and stays loaded until the process ends, as the plugins'
//! libraries do. What the window is handed: the parent, the plugin's instance and its
//! extension data (many windows reach straight into their plugin), the URID map of that
//! instance, the sample rate, and three ways to call back: a new size, a control taken hold
//! of or let go, a port's number by its symbol.
//!
//! Main thread only. The structs are the C ones of `lv2/ui/ui.h`.

use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use lv2_raw::LV2Feature;

pub const NO_PORT: u32 = u32::MAX;

/// `LV2UI_Write_Function`.
pub type WriteFn = unsafe extern "C" fn(
    controller: *mut c_void,
    port: u32,
    size: u32,
    protocol: u32,
    buffer: *const c_void,
);
pub type ResizeFn = unsafe extern "C" fn(handle: *mut c_void, width: c_int, height: c_int) -> c_int;
pub type TouchFn = unsafe extern "C" fn(handle: *mut c_void, port: u32, grabbed: bool);
pub type PortIndexFn = unsafe extern "C" fn(handle: *mut c_void, symbol: *const c_char) -> u32;
pub type ExtensionData = extern "C" fn(uri: *const c_char) -> *const c_void;

/// `LV2UI_Descriptor`.
#[repr(C)]
struct Descriptor {
    uri: *const c_char,
    instantiate: unsafe extern "C" fn(
        descriptor: *const Descriptor,
        plugin_uri: *const c_char,
        bundle_path: *const c_char,
        write: WriteFn,
        controller: *mut c_void,
        widget: *mut *mut c_void,
        features: *const *const LV2Feature,
    ) -> *mut c_void,
    cleanup: unsafe extern "C" fn(ui: *mut c_void),
    port_event: Option<
        unsafe extern "C" fn(
            ui: *mut c_void,
            port: u32,
            size: u32,
            format: u32,
            buffer: *const c_void,
        ),
    >,
    extension_data: Option<unsafe extern "C" fn(uri: *const c_char) -> *const c_void>,
}

/// `LV2UI_Idle_Interface`.
#[repr(C)]
struct IdleInterface {
    idle: unsafe extern "C" fn(ui: *mut c_void) -> c_int,
}

/// `LV2UI_Resize`, `LV2UI_Touch` and `LV2UI_Port_Map` all have this shape.
#[repr(C)]
struct Callback<F> {
    handle: *mut c_void,
    call: F,
}

/// `LV2_Extension_Data_Feature`.
#[repr(C)]
struct DataAccess {
    data_access: ExtensionData,
}

/// `LV2_Options_Option`.
#[repr(C)]
struct UiOption {
    context: u32,
    subject: u32,
    key: u32,
    size: u32,
    kind: u32,
    value: *const c_void,
}

/// What the window calls back with; `handle` is passed to each of them.
pub struct Callbacks {
    pub handle: *mut c_void,
    pub write: WriteFn,
    pub resize: ResizeFn,
    pub touch: TouchFn,
    pub port_index: PortIndexFn,
}

/// What the window is told of its plugin.
pub struct Plugin<'a> {
    pub uri: &'a str,
    /// The plugin's `LV2_Handle`.
    pub instance: *mut c_void,
    pub extension_data: Option<ExtensionData>,
    /// `urid:map` and `urid:unmap` of the plugin's instance.
    pub urid: [&'a LV2Feature; 2],
    pub sample_rate: f32,
    /// The URIDs of `param:sampleRate`, `ui:updateRate` and `atom:Float`.
    pub option_urids: [u32; 3],
}

/// What stays where it is for as long as the window: the window holds pointers to it.
struct Kept {
    resize: Callback<ResizeFn>,
    touch: Callback<TouchFn>,
    port_map: Callback<PortIndexFn>,
    data_access: Option<DataAccess>,
    numbers: [f32; 2],
    options: [UiOption; 3],
}

pub struct Ui {
    descriptor: *const Descriptor,
    handle: *mut c_void,
    idle: Option<unsafe extern "C" fn(ui: *mut c_void) -> c_int>,
    _kept: Box<Kept>,
}

/// The features a window may say it cannot do without.
pub const FEATURES: [&str; 12] = [
    "http://lv2plug.in/ns/extensions/ui#parent",
    "http://lv2plug.in/ns/extensions/ui#resize",
    "http://lv2plug.in/ns/extensions/ui#idleInterface",
    "http://lv2plug.in/ns/extensions/ui#touch",
    "http://lv2plug.in/ns/extensions/ui#portMap",
    "http://lv2plug.in/ns/extensions/ui#noUserResize",
    "http://lv2plug.in/ns/extensions/ui#fixedSize",
    "http://lv2plug.in/ns/ext/instance-access",
    "http://lv2plug.in/ns/ext/data-access",
    "http://lv2plug.in/ns/ext/urid#map",
    "http://lv2plug.in/ns/ext/urid#unmap",
    "http://lv2plug.in/ns/ext/options#options",
];

/// How often the window is told it will be called, per second.
const UPDATE_RATE: f32 = 60.0;

fn c_path(path: &Path, folder: bool) -> Result<CString, String> {
    let mut bytes = path.as_os_str().as_bytes().to_vec();
    if folder && bytes.last() != Some(&b'/') {
        bytes.push(b'/');
    }
    CString::new(bytes).map_err(|e| e.to_string())
}

impl Ui {
    /// Loads the window's library and has it make its window inside the X11 window `parent`.
    ///
    /// # Safety
    /// `plugin.instance` is the live instance of the plugin `ui_uri` belongs to, and stays
    /// alive longer than the `Ui`; `callbacks.handle` is good for the callbacks for as long.
    pub unsafe fn open(
        ui_uri: &str,
        bundle: &Path,
        binary: &Path,
        parent: u32,
        plugin: &Plugin<'_>,
        callbacks: &Callbacks,
    ) -> Result<Ui, String> {
        let binary_c = c_path(binary, false)?;
        // SAFETY: loading a plugin's library runs its code; that is what this process is
        // for. The symbol is the one LV2 names, with the signature LV2 gives it.
        let descriptor = unsafe {
            let library = libc::dlopen(binary_c.as_ptr(), libc::RTLD_NOW | libc::RTLD_LOCAL);
            if library.is_null() {
                let why = libc::dlerror();
                let why = if why.is_null() {
                    String::new()
                } else {
                    CStr::from_ptr(why).to_string_lossy().into_owned()
                };
                return Err(format!("dlopen {}: {why}", binary.display()));
            }
            let get = libc::dlsym(library, c"lv2ui_descriptor".as_ptr());
            if get.is_null() {
                return Err("no lv2ui_descriptor".into());
            }
            let get: unsafe extern "C" fn(u32) -> *const Descriptor = std::mem::transmute(get);
            let mut index = 0;
            loop {
                let descriptor = get(index);
                if descriptor.is_null() {
                    return Err(format!("no window {ui_uri} in {}", binary.display()));
                }
                let uri = (*descriptor).uri;
                if !uri.is_null() && CStr::from_ptr(uri).to_bytes() == ui_uri.as_bytes() {
                    break descriptor;
                }
                index += 1;
            }
        };

        let [sample_rate_key, update_rate_key, float] = plugin.option_urids;
        let option = |key: u32| UiOption {
            context: 0,
            subject: 0,
            key,
            size: 4,
            kind: float,
            value: std::ptr::null(),
        };
        let mut kept = Box::new(Kept {
            resize: Callback {
                handle: callbacks.handle,
                call: callbacks.resize,
            },
            touch: Callback {
                handle: callbacks.handle,
                call: callbacks.touch,
            },
            port_map: Callback {
                handle: callbacks.handle,
                call: callbacks.port_index,
            },
            data_access: plugin
                .extension_data
                .map(|data_access| DataAccess { data_access }),
            numbers: [plugin.sample_rate, UPDATE_RATE],
            options: [
                option(sample_rate_key),
                option(update_rate_key),
                UiOption {
                    context: 0,
                    subject: 0,
                    key: 0,
                    size: 0,
                    kind: 0,
                    value: std::ptr::null(),
                },
            ],
        });
        kept.options[0].value = (&raw const kept.numbers[0]).cast();
        kept.options[1].value = (&raw const kept.numbers[1]).cast();

        let feature = |uri: &'static CStr, data: *mut c_void| LV2Feature {
            uri: uri.as_ptr(),
            data,
        };
        let mut features = vec![
            feature(
                c"http://lv2plug.in/ns/extensions/ui#parent",
                parent as usize as *mut c_void,
            ),
            feature(
                c"http://lv2plug.in/ns/extensions/ui#resize",
                (&raw mut kept.resize).cast(),
            ),
            feature(
                c"http://lv2plug.in/ns/extensions/ui#idleInterface",
                std::ptr::null_mut(),
            ),
            feature(
                c"http://lv2plug.in/ns/extensions/ui#touch",
                (&raw mut kept.touch).cast(),
            ),
            feature(
                c"http://lv2plug.in/ns/extensions/ui#portMap",
                (&raw mut kept.port_map).cast(),
            ),
            feature(c"http://lv2plug.in/ns/ext/instance-access", plugin.instance),
            feature(
                c"http://lv2plug.in/ns/ext/options#options",
                kept.options.as_mut_ptr().cast(),
            ),
        ];
        if let Some(data_access) = kept.data_access.as_mut() {
            features.push(feature(
                c"http://lv2plug.in/ns/ext/data-access",
                (&raw mut *data_access).cast(),
            ));
        }
        for urid in plugin.urid {
            features.push(LV2Feature {
                uri: urid.uri,
                data: urid.data,
            });
        }
        let pointers: Vec<*const LV2Feature> = (features.iter())
            .map(|feature| &raw const *feature)
            .chain([std::ptr::null()])
            .collect();

        let plugin_uri = CString::new(plugin.uri).map_err(|e| e.to_string())?;
        let bundle_c = c_path(bundle, true)?;
        let mut widget = std::ptr::null_mut();
        // SAFETY: the descriptor is the library's own; every pointer handed in lives until
        // the call is over, and those the window may keep live in `kept`.
        let (handle, idle) = unsafe {
            let handle = ((*descriptor).instantiate)(
                descriptor,
                plugin_uri.as_ptr(),
                bundle_c.as_ptr(),
                callbacks.write,
                callbacks.handle,
                &mut widget,
                pointers.as_ptr(),
            );
            if handle.is_null() {
                return Err("the window was not made".into());
            }
            let idle = (*descriptor)
                .extension_data
                .map(|get| get(c"http://lv2plug.in/ns/extensions/ui#idleInterface".as_ptr()))
                .filter(|interface| !interface.is_null())
                .map(|interface| (*interface.cast::<IdleInterface>()).idle);
            (handle, idle)
        };
        Ok(Ui {
            descriptor,
            handle,
            idle,
            _kept: kept,
        })
    }

    /// Tells the window what a port holds now. `protocol` 0 is one `f32`; otherwise the URID
    /// of `atom:eventTransfer`, and `bytes` is one atom.
    pub fn port_event(&self, port: u32, protocol: u32, bytes: &[u8]) {
        // SAFETY: the window's own function, with a buffer good for the call.
        unsafe {
            if let Some(port_event) = (*self.descriptor).port_event {
                port_event(
                    self.handle,
                    port,
                    bytes.len() as u32,
                    protocol,
                    bytes.as_ptr().cast(),
                );
            }
        }
    }

    /// The window's turn to look after itself. False when it says it has closed.
    pub fn idle(&self) -> bool {
        match self.idle {
            // SAFETY: the window's own function.
            Some(idle) => unsafe { idle(self.handle) == 0 },
            None => true,
        }
    }
}

impl Drop for Ui {
    fn drop(&mut self) {
        // SAFETY: the way LV2 ends a window, on the main thread.
        unsafe { ((*self.descriptor).cleanup)(self.handle) }
    }
}
