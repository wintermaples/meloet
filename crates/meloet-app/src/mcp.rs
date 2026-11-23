//! `meloet --mcp`: this process becomes `meloet-mcp`, the bridge an AI app starts, found next
//! to this program. An AppImage is one file, so an AI app is told to start
//! `meloet_X.Y.Z_amd64.AppImage --mcp` where a deb has `/usr/bin/meloet-mcp`.

use std::ffi::OsString;
use std::os::unix::process::CommandExt;

const FLAG: &str = "--mcp";
const BRIDGE_EXE: &str = "meloet-mcp";

/// The arguments for the bridge when the first argument is [`FLAG`].
fn bridge_args(args: impl Iterator<Item = OsString>) -> Option<Vec<OsString>> {
    let mut args = args.skip(1);
    (args.next()? == FLAG).then(|| args.collect())
}

/// With [`FLAG`], runs the bridge in this process and never returns; else returns.
pub fn become_bridge_if_asked() {
    let Some(args) = bridge_args(std::env::args_os()) else {
        return;
    };
    let exe = match std::env::current_exe() {
        Ok(exe) => exe.with_file_name(BRIDGE_EXE),
        Err(e) => {
            eprintln!("meloet: cannot find this program, so not {BRIDGE_EXE} either: {e}");
            std::process::exit(1);
        }
    };
    let mut command = std::process::Command::new(&exe);
    command.args(args);
    meloet_plugin::launch::outside_appimage(&mut command);
    let e = command.exec();
    eprintln!("meloet: cannot start {}: {e}", exe.display());
    std::process::exit(1);
}

#[cfg(test)]
mod tests {
    use super::bridge_args;
    use std::ffi::OsString;

    fn args(list: &[&str]) -> impl Iterator<Item = OsString> {
        list.iter()
            .map(OsString::from)
            .collect::<Vec<_>>()
            .into_iter()
    }

    #[test]
    fn only_a_first_mcp_makes_the_bridge() {
        assert_eq!(bridge_args(args(&["meloet", "--mcp"])), Some(vec![]));
        assert_eq!(
            bridge_args(args(&["meloet", "--mcp", "x"])),
            Some(vec![OsString::from("x")])
        );
        assert_eq!(bridge_args(args(&["meloet"])), None);
        assert_eq!(bridge_args(args(&["meloet", "song.meloet"])), None);
        assert_eq!(bridge_args(args(&["meloet", "song.meloet", "--mcp"])), None);
        assert_eq!(bridge_args(args(&["meloet", "--mcpx"])), None);
    }
}
