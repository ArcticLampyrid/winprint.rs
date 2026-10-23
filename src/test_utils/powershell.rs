use std::{
    io,
    process::{Command, Stdio},
};

pub(super) fn quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

/// Treat cmdlet errors as failures, including those followed by a successful command.
///
/// NOTE: keep script compatible with Windows PowerShell 5.1.
pub(crate) fn run(context: &str, script: &str) -> io::Result<String> {
    let output = Command::new("powershell")
        .stdin(Stdio::null())
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            &format!("$ErrorActionPreference = 'Stop'\nSet-StrictMode -Version 2\n{script}"),
        ])
        .output()?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "PowerShell failed during {} ({}):\nstdout: \n{}\nstderr: \n{}",
            context,
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Run script sequentially across processes, using a named mutex to prevent concurrent execution.
///
/// NOTE: keep script compatible with Windows PowerShell 5.1.
pub(super) fn run_sequentially(context: &str, script: &str) -> io::Result<()> {
    run(
        context,
        &format!(
            r#"
$mutex = [System.Threading.Mutex]::new($false, 'Global\winprint-test-printer-sequential-execution')
$locked = $false
try {{
    try {{
        $locked = $mutex.WaitOne([TimeSpan]::FromMinutes(5))
    }} catch [System.Threading.AbandonedMutexException] {{
        # The previous owner died; WaitOne still granted ownership.
        $locked = $true
    }}
    if (-not $locked) {{ throw 'Timed out waiting for test printer sequential execution' }}
    & {{
{script}
    }}
}} finally {{
    if ($locked) {{ $mutex.ReleaseMutex() }}
    $mutex.Dispose()
}}
"#
        ),
    )
    .map(|output| {
        if !output.trim().is_empty() {
            eprintln!("{output}");
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cmdlet_failure_is_not_hidden_by_later_success() {
        let error = run_sequentially(
            "intentional failure",
            "Write-Error 'failed setup'; Write-Output 'success'",
        )
        .unwrap_err();
        assert!(error.to_string().contains("failed setup"));
        // A failed script must release the cross-process configuration lock.
        run_sequentially("after failed setup", "").unwrap();
    }

    #[test]
    fn quotes_single_quotes() {
        assert_eq!(quote("printer's name"), "'printer''s name'");
    }
}
