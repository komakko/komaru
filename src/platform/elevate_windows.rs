use std::ffi::{OsStr, OsString};
use std::os::windows::ffi::OsStrExt;
use std::path::Path;

use windows::Win32::Foundation::{CloseHandle, ERROR_CANCELLED};
use windows::Win32::System::Threading::{GetExitCodeProcess, INFINITE, WaitForSingleObject};
use windows::Win32::UI::Shell::{SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW};
use windows::core::{HRESULT, PCWSTR, w};

use crate::core::update::{Download, Staging};
use crate::error::UpdateError;

const FAILED: u32 = 1;

pub fn replace_executable(executable: &Path, download: &Download) -> Result<(), UpdateError> {
    let staging = Staging::create(download)?;
    let parameters = parameters(&staging.build(), &staging.signature());
    match run_elevated(executable, &parameters)? {
        0 => Ok(()),
        code => Err(UpdateError::Elevated(code)),
    }
}

fn parameters(build: &Path, signature: &Path) -> Vec<u16> {
    let mut text = OsString::from("--apply-update \"");
    text.push(build);
    text.push("\" \"");
    text.push(signature);
    text.push("\"");
    wide(&text)
}

fn wide(text: &OsStr) -> Vec<u16> {
    text.encode_wide().chain(Some(0)).collect()
}

fn run_elevated(executable: &Path, parameters: &[u16]) -> Result<u32, UpdateError> {
    let file = wide(executable.as_os_str());
    let mut request = SHELLEXECUTEINFOW {
        cbSize: size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS,
        lpVerb: w!("runas"),
        lpFile: PCWSTR(file.as_ptr()),
        lpParameters: PCWSTR(parameters.as_ptr()),
        ..unsafe { std::mem::zeroed() }
    };
    unsafe { ShellExecuteExW(&mut request) }.map_err(launch_error)?;
    let mut code = FAILED;
    unsafe {
        WaitForSingleObject(request.hProcess, INFINITE);
        let _ = GetExitCodeProcess(request.hProcess, &mut code);
        let _ = CloseHandle(request.hProcess);
    }
    Ok(code)
}

fn launch_error(error: windows::core::Error) -> UpdateError {
    if error.code() == HRESULT::from_win32(ERROR_CANCELLED.0) {
        UpdateError::Declined
    } else {
        UpdateError::Elevate(error.to_string())
    }
}
