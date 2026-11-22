//! Development only: when the process gets a fatal signal, writes which signal, at what
//! address, on which thread and the stack (the libraries and their offsets) to standard
//! error, then dies of the signal as it would have. Only with `--log`: without it standard
//! error is `/dev/null`.
//!
//! What the handler does is not all async-signal-safe (glibc's `backtrace` may load libgcc
//! the first time), so it is called once here before any plugin runs.

use std::ffi::c_void;

const SIGNALS: [libc::c_int; 5] = [
    libc::SIGSEGV,
    libc::SIGBUS,
    libc::SIGILL,
    libc::SIGFPE,
    libc::SIGABRT,
];

fn put(text: &[u8]) {
    // SAFETY: a plain write of a valid buffer.
    unsafe { libc::write(2, text.as_ptr().cast(), text.len()) };
}

fn put_hex(mut value: usize) {
    let mut buf = [0u8; 18];
    let mut at = buf.len();
    loop {
        at -= 1;
        buf[at] = b"0123456789abcdef"[value & 15];
        value >>= 4;
        if value == 0 {
            break;
        }
    }
    at -= 2;
    buf[at] = b'0';
    buf[at + 1] = b'x';
    put(&buf[at..]);
}

extern "C" fn on_signal(signal: libc::c_int, info: *mut libc::siginfo_t, _: *mut c_void) {
    put(b"\nplughost: fatal signal ");
    put_hex(signal as usize);
    put(b" code ");
    // SAFETY: the kernel hands a valid siginfo to an SA_SIGINFO handler.
    let (code, addr) = unsafe { ((*info).si_code, (*info).si_addr() as usize) };
    put_hex(code as usize);
    put(b" addr ");
    put_hex(addr);
    put(b" thread ");
    let mut name = [0u8; 16];
    // SAFETY: PR_GET_NAME writes at most 16 bytes.
    unsafe { libc::prctl(libc::PR_GET_NAME, name.as_mut_ptr()) };
    let len = name.iter().position(|&b| b == 0).unwrap_or(16);
    put(&name[..len]);
    put(b" tid ");
    // SAFETY: a plain call.
    put_hex(unsafe { libc::gettid() } as usize);
    put(b"\n");
    let mut frames = [std::ptr::null_mut::<c_void>(); 64];
    // SAFETY: the buffer holds 64 entries.
    unsafe {
        let n = libc::backtrace(frames.as_mut_ptr(), 64);
        libc::backtrace_symbols_fd(frames.as_ptr(), n, 2);
    }
    put(b"plughost: end of stack\n");
    // SA_RESETHAND put the default action back: returning runs the faulting instruction
    // again (or, for a raised signal, raise it once more) and the process dies of it.
    if code <= 0 {
        // SAFETY: a plain call.
        unsafe { libc::raise(signal) };
    }
}

pub fn install() {
    let mut frames = [std::ptr::null_mut::<c_void>(); 4];
    // SAFETY: see the module; a plain sigaction with a handler of the right shape.
    unsafe {
        libc::backtrace(frames.as_mut_ptr(), 4);
        for signal in SIGNALS {
            let mut action: libc::sigaction = std::mem::zeroed();
            action.sa_sigaction = on_signal as *const () as usize;
            action.sa_flags = libc::SA_SIGINFO | libc::SA_ONSTACK | libc::SA_RESETHAND;
            libc::sigemptyset(&mut action.sa_mask);
            libc::sigaction(signal, &action, std::ptr::null_mut());
        }
    }
}
