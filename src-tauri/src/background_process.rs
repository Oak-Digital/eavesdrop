use std::{ffi::OsStr, process::Command};

/// Build a background command without allocating a console on Windows.
/// Redirecting stdout alone does not prevent console windows in a GUI app.
pub fn command(program: impl AsRef<OsStr>) -> Command {
    let mut command = Command::new(program);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    // Set this on every platform so commands cannot wait for terminal input.
    // Callers such as the summary engine can explicitly supply a stdin pipe.
    command.stdin(std::process::Stdio::null());
    command
}

#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::*;
    use std::{io::Write, process::Stdio};

    #[test]
    fn background_child_has_no_console_and_preserves_piped_io() {
        let mut child = command("powershell.exe")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                r#"$ErrorActionPreference = 'Stop';
Add-Type -TypeDefinition 'using System; using System.Runtime.InteropServices; public class ConsoleProbe { [DllImport("kernel32.dll")] public static extern IntPtr GetConsoleWindow(); }';
[Console]::Out.WriteLine([ConsoleProbe]::GetConsoleWindow().ToInt64());
[Console]::Out.Write([Console]::In.ReadToEnd());
[Console]::Error.Write('diagnostic');"#,
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(b"request").unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success(), "{:?}", output);
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n"),
            "0\nrequest"
        );
        assert_eq!(output.stderr, b"diagnostic");
    }

    #[test]
    fn background_tasklist_still_returns_processes() {
        let output = command("tasklist.exe")
            .args(["/FO", "CSV", "/NH"])
            .output()
            .unwrap();
        assert!(output.status.success(), "{:?}", output);
        assert!(
            String::from_utf8_lossy(&output.stdout)
                .to_lowercase()
                .contains("tasklist.exe")
        );
    }
}
