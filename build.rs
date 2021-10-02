fn main() {
    windows::build! {
        Windows::Win32::Graphics::Gdi::{CreateDCW, DeleteDC},
        Windows::Win32::Graphics::Printing::*,
        Windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoUninitialize},
        Windows::Win32::Storage::Xps::*,
        Windows::Win32::Storage::Xps::Printing::*,
        Windows::Win32::Foundation::CloseHandle,
        Windows::Win32::System::Threading::{CreateEventW, SetEvent, WaitForSingleObject},
        Windows::Win32::System::WindowsProgramming::INFINITE
    };
}
