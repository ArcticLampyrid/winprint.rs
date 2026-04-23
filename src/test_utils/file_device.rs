use crate::printer::PrinterDevice;
use sha2::{Digest, Sha256};
use std::{
    cell::OnceCell,
    marker::PhantomData,
    path::PathBuf,
    process::{Command, Stdio},
    sync::OnceLock,
};

/// Defines a virtual "print-to-file" device backed by a built-in Windows driver.
///
/// Implement this trait to expose additional file-based drivers. At present, two built-in
/// providers are shipped: [`PwgRaster`] and [`Pdf`].
pub trait FilePrinterProvider: Send + Sync + 'static {
    /// Returns the Windows driver name (as accepted by `Add-Printer -DriverName`) to use for the
    /// virtual printer.
    fn driver_name() -> &'static str;
}

/// A built-in [`FilePrinterProvider`] backed by the `Microsoft PWG Raster Class Driver`.
pub struct PwgRaster;
impl FilePrinterProvider for PwgRaster {
    fn driver_name() -> &'static str {
        "Microsoft PWG Raster Class Driver"
    }
}

/// A built-in [`FilePrinterProvider`] backed by the `Microsoft Print To PDF` driver.
pub struct Pdf;
impl FilePrinterProvider for Pdf {
    fn driver_name() -> &'static str {
        "Microsoft Print To PDF"
    }
}

fn ps_quote(s: &str) -> String {
    // PowerShell single-quoted string: escape ' by doubling it
    format!("'{}'", s.replace('\'', "''"))
}

fn run_powershell(script: &str) {
    let status = Command::new("powershell")
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .args(["-NoProfile", "-Command", script])
        .spawn()
        .expect("failed to spawn powershell")
        .wait()
        .expect("failed to wait powershell");
    // We don't propagate non-zero statuses here: the individual cmdlets use
    // `-ErrorAction Continue`/`SilentlyContinue` and the caller verifies via
    // `PrinterDevice::all()` after the fact.
    let _ = status;
}

fn printer_name_for(port_path: &str) -> String {
    // printer name = file-device-{pid}-{bs58(sha256(port_path))}
    let hash = Sha256::digest(port_path.as_bytes());
    format!(
        "file-device-{}-{}",
        std::process::id(),
        bs58::encode(hash).into_string()
    )
}

fn make_temp_port_path() -> PathBuf {
    // Unique per-device file. We rely on (pid, atomic counter, nanos) to keep it unique without
    // needing an extra crate.
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let mut p = std::env::temp_dir();
    p.push(format!(
        "winprint-file-device-{}-{}-{}.prn",
        std::process::id(),
        n,
        nanos
    ));
    p
}

/// The one-shot cleanup / driver-install script.
///
/// On first use per-process we:
///   * ensure the driver is installed,
///   * scan for leftover `file-device-*` printers whose owning PID no longer exists and remove
///     them along with their ports (and the backing temp files, on the Rust side we will also try
///     to delete files on Drop).
fn ensure_driver_and_cleanup<T: FilePrinterProvider>() {
    let driver = T::driver_name();
    // NOTE: we have to keep this script compatible with Windows PowerShell 5.1.
    let script = format!(
        r#"
$ErrorActionPreference = 'Continue'

# Install driver if missing.
if (-not (Get-PrinterDriver -Name {driver_q} -ErrorAction SilentlyContinue)) {{
    Add-PrinterDriver -Name {driver_q} -ErrorAction Continue
}}

# Minimal base58 decoder (Bitcoin alphabet) — adapted from
# https://gist.github.com/gkostoulias/9e0af1595aaf5e6728443497a7defbe5
function Convert-FromBase58 {{
    param([string]$s)
    $alphabet = '123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz'
    $num = [System.Numerics.BigInteger]::Zero
    foreach ($c in $s.ToCharArray()) {{
        $i = $alphabet.IndexOf($c)
        if ($i -lt 0) {{ return $null }}
        $num = ($num * [System.Numerics.BigInteger]58) + [System.Numerics.BigInteger]$i
    }}
    $bytes = $num.ToByteArray()
    # BigInteger is little-endian, possibly with a trailing sign byte. Strip trailing 0x00
    # (produced for positive numbers whose top bit would otherwise be set) and reverse.
    if ($bytes.Length -gt 1 -and $bytes[$bytes.Length - 1] -eq 0) {{
        $bytes = $bytes[0..($bytes.Length - 2)]
    }}
    [array]::Reverse($bytes)
    # Add leading zero bytes for each leading '1' in input (base58 convention).
    $leading = 0
    foreach ($c in $s.ToCharArray()) {{
        if ($c -eq '1') {{ $leading++ }} else {{ break }}
    }}
    if ($leading -gt 0) {{
        $pad = New-Object byte[] $leading
        $bytes = $pad + $bytes
    }}
    return ,$bytes
}}

$sha256 = [System.Security.Cryptography.SHA256]::Create()

Get-Printer -ErrorAction SilentlyContinue | Where-Object {{
    $_.Type -eq 'Local' -and $_.DriverName -eq {driver_q} -and $_.Name -match '^file-device-(\d+)-([1-9A-HJ-NP-Za-km-z]+)$'
}} | ForEach-Object {{
    $p = $_
    $m = [regex]::Match($p.Name, '^file-device-(\d+)-([1-9A-HJ-NP-Za-km-z]+)$')
    $pid_str = $m.Groups[1].Value
    $hash_b58 = $m.Groups[2].Value

    # If the owning process is still alive, leave it alone.
    $alive = $true
    try {{
        $proc = Get-Process -Id ([int]$pid_str) -ErrorAction Stop
        if (-not $proc) {{ $alive = $false }}
    }} catch {{
        $alive = $false
    }}
    if ($alive) {{ return }}

    # Verify the bs58(sha256(port_name)) matches the name so that we only remove printers that we
    # created. If anything mismatches, err on the side of caution and skip.
    $portName = $p.PortName
    if (-not $portName) {{ return }}
    $expected = $sha256.ComputeHash([System.Text.Encoding]::UTF8.GetBytes($portName))
    $actual = Convert-FromBase58 $hash_b58
    if ($null -eq $actual -or $actual.Length -ne $expected.Length) {{ return }}
    $eq = $true
    for ($i = 0; $i -lt $expected.Length; $i++) {{
        if ($actual[$i] -ne $expected[$i]) {{ $eq = $false; break }}
    }}
    if (-not $eq) {{ return }}

    Remove-Printer -Name $p.Name -ErrorAction Continue
    if (Get-PrinterPort -Name $portName -ErrorAction SilentlyContinue) {{
        Remove-PrinterPort -Name $portName -ErrorAction Continue
    }}
    if (Test-Path -LiteralPath $portName) {{
        Remove-Item -LiteralPath $portName -Force -ErrorAction Continue
    }}
}}
"#,
        driver_q = ps_quote(driver),
    );
    run_powershell(&script);
}

/// A thread-local auto-managed virtual "print-to-file" device.
///
/// Each call to [`FilePrinterDevice::thread_local`] installs (on first use on that thread) a
/// unique printer backed by [`FilePrinterProvider::driver_name()`] and a freshly-created temp
/// file acting as its port. The device — and the underlying printer/port/file — are automatically
/// torn down when the thread exits.
///
/// ```no_run
/// use winprint::test_utils::file_device::{FilePrinterDevice, PwgRaster};
/// let device = FilePrinterDevice::<PwgRaster>::thread_local();
/// ```
pub struct FilePrinterDevice<T: FilePrinterProvider> {
    _phantom: PhantomData<fn() -> T>,
}

struct FilePrinterDeviceInner<T: FilePrinterProvider> {
    printer: PrinterDevice,
    port_path: PathBuf,
    _phantom: PhantomData<fn() -> T>,
}

impl<T: FilePrinterProvider> FilePrinterDeviceInner<T> {
    fn new() -> Self {
        // One-shot per (process, driver) initialization and leftover cleanup.
        init_once::<T>();

        let port_path = make_temp_port_path();
        let port_str = port_path.to_string_lossy().into_owned();
        let printer_name = printer_name_for(&port_str);

        // If the printer already exists (shouldn't, given the random temp path, but be safe),
        // reuse it.
        if let Some(printer) = PrinterDevice::all()
            .ok()
            .and_then(|all| all.into_iter().find(|p| p.name() == printer_name))
        {
            return FilePrinterDeviceInner {
                printer,
                port_path,
                _phantom: PhantomData,
            };
        }

        let driver = T::driver_name();
        let script = format!(
            r#"
$ErrorActionPreference = 'Continue'
if (-not (Get-PrinterPort -Name {port_q} -ErrorAction SilentlyContinue)) {{
    Add-PrinterPort -Name {port_q} -ErrorAction Continue
}}
Add-Printer -Name {name_q} -PortName {port_q} -DriverName {driver_q} -ErrorAction Continue
"#,
            port_q = ps_quote(&port_str),
            name_q = ps_quote(&printer_name),
            driver_q = ps_quote(driver),
        );
        run_powershell(&script);

        let printer = PrinterDevice::all()
            .expect("failed to enumerate printers")
            .into_iter()
            .find(|p| p.name() == printer_name)
            .unwrap_or_else(|| {
                panic!(
                    "failed to create file printer device (name={}, port={}, driver={})",
                    printer_name, port_str, driver
                )
            });
        FilePrinterDeviceInner {
            printer,
            port_path,
            _phantom: PhantomData,
        }
    }
}

impl<T: FilePrinterProvider> Drop for FilePrinterDeviceInner<T> {
    fn drop(&mut self) {
        let port_str = self.port_path.to_string_lossy().into_owned();
        let script = format!(
            r#"
$ErrorActionPreference = 'Continue'
Remove-Printer -Name {name_q} -ErrorAction Continue
if (Get-PrinterPort -Name {port_q} -ErrorAction SilentlyContinue) {{
    Remove-PrinterPort -Name {port_q} -ErrorAction Continue
}}
"#,
            name_q = ps_quote(self.printer.name()),
            port_q = ps_quote(&port_str),
        );
        run_powershell(&script);

        // Best-effort remove of the backing file.
        let _ = std::fs::remove_file(&self.port_path);
    }
}

// We need a distinct thread_local + init_once per provider type. Use a generic helper struct
// with `OnceLock` for the init flag, keyed on the provider's driver name via a `LazyLock`-ish
// registry implemented with a static `OnceLock<HashSet<&'static str>>` guarded by a mutex is
// overkill; instead, since providers are distinct monomorphizations we can use a const
// `OnceLock<()>` inside an associated function keyed by T via a helper trait.

thread_local! {
    static TLS_SLOT: std::cell::RefCell<TlsRegistry> = std::cell::RefCell::new(TlsRegistry::default());
}

#[derive(Default)]
struct TlsRegistry {
    entries: Vec<(std::any::TypeId, Box<dyn std::any::Any>)>,
}

fn init_once<T: FilePrinterProvider>() {
    use std::sync::Mutex;
    // Per-(process, driver) init flag. Keyed by TypeId so each provider initializes exactly once.
    static INIT: OnceLock<Mutex<std::collections::HashSet<std::any::TypeId>>> = OnceLock::new();
    let set = INIT.get_or_init(|| Mutex::new(std::collections::HashSet::new()));
    let tid = std::any::TypeId::of::<T>();
    let mut guard = set.lock().expect("init_once mutex poisoned");
    if guard.insert(tid) {
        // Drop guard before running the (slow) cleanup so we don't deadlock other threads that
        // just need to check the flag.
        drop(guard);
        ensure_driver_and_cleanup::<T>();
    }
}

impl<T: FilePrinterProvider> FilePrinterDevice<T> {
    /// Get the thread-local printer device for provider `T`.
    ///
    /// The device is created on first call and automatically removed when the calling thread
    /// exits. A leftover from a previous (crashed) process with the same driver is cleaned up
    /// opportunistically on first call per process.
    pub fn thread_local() -> PrinterDevice {
        TLS_SLOT.with(|cell| {
            let mut reg = cell.borrow_mut();
            let tid = std::any::TypeId::of::<T>();
            if let Some((_, any)) = reg.entries.iter().find(|(id, _)| *id == tid) {
                let slot = any
                    .downcast_ref::<OnceCell<FilePrinterDeviceInner<T>>>()
                    .expect("TLS registry slot had wrong type");
                return slot
                    .get_or_init(FilePrinterDeviceInner::<T>::new)
                    .printer
                    .clone();
            }
            let slot: OnceCell<FilePrinterDeviceInner<T>> = OnceCell::new();
            slot.get_or_init(FilePrinterDeviceInner::<T>::new);
            reg.entries.push((tid, Box::new(slot)));
            // Re-borrow to return the just-inserted entry.
            let (_, any) = reg.entries.last().unwrap();
            let slot = any
                .downcast_ref::<OnceCell<FilePrinterDeviceInner<T>>>()
                .expect("TLS registry slot had wrong type");
            slot.get().unwrap().printer.clone()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::printer::PrinterDevice;

    fn printer_exists(name: &str) -> bool {
        PrinterDevice::all()
            .unwrap()
            .into_iter()
            .any(|p| p.name() == name)
    }

    #[test]
    fn pwg_raster_device_roundtrip() {
        // In its own thread so the Drop happens inside the test.
        let name = std::thread::spawn(|| {
            let device = FilePrinterDevice::<PwgRaster>::thread_local();
            let name = device.name().to_string();
            assert!(
                name.starts_with("file-device-"),
                "unexpected printer name: {name}"
            );
            assert!(printer_exists(&name), "printer {name} was not created");
            name
        })
        .join()
        .unwrap();

        // After the thread exits, the Drop impl should have removed the printer.
        assert!(
            !printer_exists(&name),
            "printer {name} still present after thread exit"
        );
    }

    #[test]
    fn pdf_device_roundtrip() {
        let name = std::thread::spawn(|| {
            let device = FilePrinterDevice::<Pdf>::thread_local();
            let name = device.name().to_string();
            assert!(printer_exists(&name), "printer {name} was not created");
            name
        })
        .join()
        .unwrap();
        assert!(
            !printer_exists(&name),
            "printer {name} still present after thread exit"
        );
    }

    #[test]
    fn printer_name_is_stable_for_same_port() {
        let n1 = printer_name_for(r"C:\Temp\foo.prn");
        let n2 = printer_name_for(r"C:\Temp\foo.prn");
        assert_eq!(n1, n2);
        let n3 = printer_name_for(r"C:\Temp\bar.prn");
        assert_ne!(n1, n3);
    }
}
