use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;
use windows_sys::Win32::System::LibraryLoader::*;

static CLASS_NAME: &str = "HdawPluginWindow\0";
static INITIALIZED: std::sync::Once = std::sync::Once::new();

pub struct NativeWindow {
    pub hwnd: HWND,
}

impl NativeWindow {
    pub fn new(title: &str, width: i32, height: i32, parent: HWND) -> Result<Self, String> {
        INITIALIZED.call_once(|| {
            unsafe {
                let h_instance = GetModuleHandleW(std::ptr::null());
                let class_name_w: Vec<u16> = CLASS_NAME.encode_utf16().collect();
                
                let wnd_class = WNDCLASSEXW {
                    cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                    style: CS_HREDRAW | CS_VREDRAW,
                    lpfnWndProc: Some(wnd_proc),
                    cbClsExtra: 0,
                    cbWndExtra: 0,
                    hInstance: h_instance as _,
                    hIcon: std::ptr::null_mut(),
                    hCursor: LoadCursorW(std::ptr::null_mut(), IDC_ARROW as _),
                    hbrBackground: std::ptr::null_mut(),
                    lpszMenuName: std::ptr::null(),
                    lpszClassName: class_name_w.as_ptr(),
                    hIconSm: std::ptr::null_mut(),
                };
                
                RegisterClassExW(&wnd_class);
            }
        });

        unsafe {
            let title_w: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
            let class_name_w: Vec<u16> = CLASS_NAME.encode_utf16().collect();
            
            let style = if parent == std::ptr::null_mut() {
                WS_OVERLAPPEDWINDOW | WS_VISIBLE
            } else {
                WS_CHILD | WS_VISIBLE | WS_BORDER | WS_CAPTION | WS_THICKFRAME
            };

            let hwnd = CreateWindowExW(
                0,
                class_name_w.as_ptr(),
                title_w.as_ptr(),
                style,
                CW_USEDEFAULT, CW_USEDEFAULT,
                width, height,
                parent,
                std::ptr::null_mut(),
                GetModuleHandleW(std::ptr::null()) as _,
                std::ptr::null(),
            );

            if hwnd == std::ptr::null_mut() {
                return Err("Failed to create window".to_string());
            }

            Ok(Self { hwnd })
        }
    }

    pub fn set_title(&self, title: &str) {
        unsafe {
            let title_w: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
            SetWindowTextW(self.hwnd, title_w.as_ptr());
        }
    }

    pub fn resize(&self, width: i32, height: i32) {
        unsafe {
            SetWindowPos(self.hwnd, std::ptr::null_mut(), 0, 0, width, height, SWP_NOMOVE | SWP_NOZORDER);
        }
    }

    pub fn close(&self) {
        unsafe {
            DestroyWindow(self.hwnd);
        }
    }
}

extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match msg {
            WM_SIZE => {
                let width = (lparam & 0xFFFF) as i32;
                let height = ((lparam >> 16) & 0xFFFF) as i32;
                let child = GetWindow(hwnd, GW_CHILD);
                if child != std::ptr::null_mut() {
                    MoveWindow(child, 0, 0, width, height, 1);
                }
                0
            }
            WM_CLOSE => {
                ShowWindow(hwnd, SW_HIDE);
                0
            }
            WM_DESTROY => {
                0
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}
