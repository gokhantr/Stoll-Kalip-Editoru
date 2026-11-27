use std::sync::{Arc, Mutex};

pub const CURRENT_VERSION: &str = concat!("v", env!("CARGO_PKG_VERSION"));
pub const GITHUB_REPO_URL: &str = "https://github.com/gokhantr/Stoll-Kalip-Editoru";
pub const GITHUB_API_URL: &str =
    "https://api.github.com/repos/gokhantr/Stoll-Kalip-Editoru/releases/latest";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseInfo {
    pub tag_name: String,
    pub html_url: String,
    pub download_url: String,
    pub body_notes: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateStatus {
    Idle,
    Checking,
    UpToDate,
    UpdateAvailable(ReleaseInfo),
    Unavailable(String),
}

impl Default for UpdateStatus {
    fn default() -> Self {
        Self::Idle
    }
}

/// Spawn a background thread to check GitHub's latest release via native `winhttp.dll`.
/// Never spawns `powershell.exe` or `curl.exe`.
pub fn spawn_update_check<F>(status: Arc<Mutex<UpdateStatus>>, on_done: F)
where
    F: FnOnce() + Send + 'static,
{
    {
        let mut guard = match status.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        if matches!(*guard, UpdateStatus::Checking) {
            return;
        }
        *guard = UpdateStatus::Checking;
    }

    std::thread::spawn(move || {
        let next = match fetch_latest_release() {
            Ok(info) => {
                if is_newer_version(&info.tag_name, CURRENT_VERSION) {
                    UpdateStatus::UpdateAvailable(info)
                } else {
                    UpdateStatus::UpToDate
                }
            }
            Err(err) => UpdateStatus::Unavailable(err),
        };
        let mut guard = match status.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        *guard = next;
        drop(guard);
        on_done();
    });
}

/// Validate that a URL uses an explicit `https://` or `http://` scheme before invoking ShellExecuteW.
pub fn is_safe_http_url(url: &str) -> bool {
    let trimmed = url.trim();
    trimmed.starts_with("https://") || trimmed.starts_with("http://")
}

/// Fetch and parse GitHub latest release metadata using `GITHUB_API_URL`.
pub fn fetch_latest_release() -> Result<ReleaseInfo, String> {
    let body = winhttp_get_https(
        "api.github.com",
        "/repos/gokhantr/Stoll-Kalip-Editoru/releases/latest",
    )?;
    parse_github_release_json(&body)
}

/// Format raw Markdown release notes from GitHub's `body` field into a clean, bounded summary.
pub fn format_release_notes_summary(body: &str, max_lines: usize, max_chars: usize) -> String {
    let normalized = body.replace("\r\n", "\n");
    let mut cleaned = String::with_capacity(normalized.len());
    let mut rest = normalized.as_str();
    while let Some(start) = rest.find("<!--") {
        cleaned.push_str(&rest[..start]);
        if let Some(end_rel) = rest[start + 4..].find("-->") {
            rest = &rest[start + 4 + end_rel + 3..];
        } else {
            rest = "";
            break;
        }
    }
    cleaned.push_str(rest);

    let mut lines: Vec<String> = Vec::new();
    for raw_line in cleaned.lines() {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let line = if let Some(heading) = trimmed.strip_prefix("###") {
            heading.trim().to_string()
        } else if let Some(heading) = trimmed.strip_prefix("##") {
            heading.trim().to_string()
        } else if let Some(heading) = trimmed.strip_prefix('#') {
            heading.trim().to_string()
        } else if let Some(bullet) = trimmed.strip_prefix("- ").or_else(|| trimmed.strip_prefix("* ")) {
            format!("• {}", bullet.trim())
        } else {
            trimmed.to_string()
        };
        let no_bold = line.replace("**", "");
        if !no_bold.is_empty() {
            lines.push(no_bold);
            if lines.len() >= max_lines {
                break;
            }
        }
    }

    let joined = lines.join("\n");
    if joined.chars().count() <= max_chars {
        return joined;
    }
    let mut out: String = joined.chars().take(max_chars.saturating_sub(3)).collect();
    out.push_str("...");
    out
}

/// Open a URL in the user's default browser without spawning a shell process.
#[cfg(windows)]
pub fn open_url_in_browser(url: &str) {
    if !is_safe_http_url(url) {
        return;
    }
    #[link(name = "shell32")]
    extern "system" {
        fn ShellExecuteW(
            hwnd: *mut core::ffi::c_void,
            lp_operation: *const u16,
            lp_file: *const u16,
            lp_parameters: *const u16,
            lp_directory: *const u16,
            n_show_cmd: i32,
        ) -> isize;
    }

    let op = to_wide_null("open");
    let target = to_wide_null(url);
    unsafe {
        let _ = ShellExecuteW(
            core::ptr::null_mut(),
            op.as_ptr(),
            target.as_ptr(),
            core::ptr::null(),
            core::ptr::null(),
            1, // SW_SHOWNORMAL
        );
    }
}

#[cfg(not(windows))]
pub fn open_url_in_browser(_url: &str) {}

/// Open a local file (e.g. printable HTML report) with its default handler without spawning `cmd.exe`.
#[cfg(windows)]
pub fn open_local_file(path: &std::path::Path) {
    #[link(name = "shell32")]
    extern "system" {
        fn ShellExecuteW(
            hwnd: *mut core::ffi::c_void,
            lp_operation: *const u16,
            lp_file: *const u16,
            lp_parameters: *const u16,
            lp_directory: *const u16,
            n_show_cmd: i32,
        ) -> isize;
    }

    let op = to_wide_null("open");
    let target = to_wide_null(&path.to_string_lossy());
    unsafe {
        let _ = ShellExecuteW(
            core::ptr::null_mut(),
            op.as_ptr(),
            target.as_ptr(),
            core::ptr::null(),
            core::ptr::null(),
            1, // SW_SHOWNORMAL
        );
    }
}

#[cfg(not(windows))]
pub fn open_local_file(_path: &std::path::Path) {}

/// Compare semantic version tags (e.g. `"v1.0.1"` > `"v1.0.0"`).
pub fn is_newer_version(latest_tag: &str, current_version: &str) -> bool {
    let parse = |s: &str| -> Option<Vec<u64>> {
        let trimmed = s.trim().trim_start_matches(['v', 'V']);
        let core = trimmed.split(['-', '+']).next().unwrap_or(trimmed);
        if core.is_empty() {
            return None;
        }
        let mut parts = Vec::new();
        for seg in core.split('.') {
            parts.push(seg.parse::<u64>().ok()?);
        }
        Some(parts)
    };

    match (parse(latest_tag), parse(current_version)) {
        (Some(l), Some(c)) => {
            let max_len = l.len().max(c.len());
            for i in 0..max_len {
                let lv = l.get(i).copied().unwrap_or(0);
                let cv = c.get(i).copied().unwrap_or(0);
                if lv != cv {
                    return lv > cv;
                }
            }
            false
        }
        _ => {
            let lt = latest_tag.trim();
            let ct = current_version.trim();
            !lt.is_empty() && !lt.eq_ignore_ascii_case(ct)
        }
    }
}

/// Parse `tag_name`, `html_url`, `body`, and `.exe` `browser_download_url` from GitHub Release JSON.
pub fn parse_github_release_json(json: &str) -> Result<ReleaseInfo, String> {
    let mut tag_name: Option<String> = None;
    let mut html_url: Option<String> = None;
    let mut body_notes: Option<String> = None;
    let mut asset_urls: Vec<String> = Vec::new();

    let bytes = json.as_bytes();
    let len = bytes.len();
    let mut i = 0usize;
    let mut depth = 0i32;
    let mut assets_array_depth: Option<i32> = None;
    let mut pending_key: Option<(String, i32)> = None;
    let mut expecting_value = false;

    while i < len {
        let b = bytes[i];
        match b {
            b' ' | b'\t' | b'\r' | b'\n' => {
                i += 1;
            }
            b'{' => {
                depth += 1;
                pending_key = None;
                expecting_value = false;
                i += 1;
            }
            b'}' => {
                depth -= 1;
                expecting_value = false;
                i += 1;
            }
            b'[' => {
                if expecting_value {
                    if let Some((ref k, kd)) = pending_key {
                        if k == "assets" && kd == 1 {
                            assets_array_depth = Some(depth + 1);
                        }
                    }
                    pending_key = None;
                    expecting_value = false;
                }
                depth += 1;
                i += 1;
            }
            b']' => {
                if assets_array_depth == Some(depth) {
                    assets_array_depth = None;
                }
                depth -= 1;
                expecting_value = false;
                i += 1;
            }
            b':' => {
                expecting_value = true;
                i += 1;
            }
            b',' => {
                pending_key = None;
                expecting_value = false;
                i += 1;
            }
            b'"' => {
                let (s, next_i) = decode_json_string(json, i)?;
                i = next_i;
                if expecting_value {
                    if let Some((key, kd)) = pending_key.take() {
                        if kd == 1 {
                            match key.as_str() {
                                "tag_name" => tag_name = Some(s),
                                "html_url" => html_url = Some(s),
                                "body" => body_notes = Some(s),
                                _ => {}
                            }
                        } else if assets_array_depth.is_some()
                            && kd >= 2
                            && key == "browser_download_url"
                            && !s.is_empty()
                        {
                            asset_urls.push(s);
                        }
                    }
                    expecting_value = false;
                } else {
                    pending_key = Some((s, depth));
                }
            }
            _ => {
                if expecting_value {
                    pending_key = None;
                    expecting_value = false;
                }
                i += 1;
            }
        }
    }

    let tag = tag_name
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| "GitHub yanıtında sürüm etiketi (tag_name) bulunamadı.".to_string())?;
    let html = html_url
        .filter(|s| !s.trim().is_empty() && is_safe_http_url(s))
        .unwrap_or_else(|| GITHUB_REPO_URL.to_string());
    let exe_url = asset_urls
        .iter()
        .find(|u| is_safe_http_url(u) && u.to_ascii_lowercase().ends_with(".exe"))
        .or_else(|| {
            asset_urls
                .iter()
                .find(|u| is_safe_http_url(u) && u.to_ascii_lowercase().ends_with(".zip"))
        })
        .cloned()
        .unwrap_or_else(|| html.clone());

    Ok(ReleaseInfo {
        tag_name: tag,
        html_url: html,
        download_url: exe_url,
        body_notes: body_notes.unwrap_or_default(),
    })
}

fn decode_json_string(json: &str, start_quote: usize) -> Result<(String, usize), String> {
    let bytes = json.as_bytes();
    let mut out = String::new();
    let mut i = start_quote + 1;

    while i < bytes.len() {
        let b = bytes[i];
        if b == b'"' {
            return Ok((out, i + 1));
        }
        if b == b'\\' {
            i += 1;
            if i >= bytes.len() {
                return Err("Geçersiz JSON kaçış dizisi.".to_string());
            }
            match bytes[i] {
                b'"' => {
                    out.push('"');
                    i += 1;
                }
                b'\\' => {
                    out.push('\\');
                    i += 1;
                }
                b'/' => {
                    out.push('/');
                    i += 1;
                }
                b'b' => {
                    out.push('\u{0008}');
                    i += 1;
                }
                b'f' => {
                    out.push('\u{000C}');
                    i += 1;
                }
                b'n' => {
                    out.push('\n');
                    i += 1;
                }
                b'r' => {
                    out.push('\r');
                    i += 1;
                }
                b't' => {
                    out.push('\t');
                    i += 1;
                }
                b'u' => {
                    let hex = json
                        .get(i + 1..i + 5)
                        .ok_or_else(|| "Eksik \\uXXXX dizisi.".to_string())?;
                    let code = u16::from_str_radix(hex, 16)
                        .map_err(|_| "Geçersiz \\uXXXX dizisi.".to_string())?;
                    i += 5;
                    if (0xD800..=0xDBFF).contains(&code) {
                        if json.as_bytes().get(i..i + 2) == Some(b"\\u") {
                            if let Some(low_hex) = json.get(i + 2..i + 6) {
                                if let Ok(low) = u16::from_str_radix(low_hex, 16) {
                                    if (0xDC00..=0xDFFF).contains(&low) {
                                        let cp = 0x10000
                                            + (((code as u32 - 0xD800) << 10)
                                                | (low as u32 - 0xDC00));
                                        if let Some(ch) = char::from_u32(cp) {
                                            out.push(ch);
                                        }
                                        i += 6;
                                        continue;
                                    }
                                }
                            }
                        }
                    }
                    if let Some(ch) = char::from_u32(code as u32) {
                        out.push(ch);
                    }
                }
                other => {
                    out.push(other as char);
                    i += 1;
                }
            }
        } else {
            let ch = json[i..]
                .chars()
                .next()
                .ok_or_else(|| "Geçersiz UTF-8 JSON karakteri.".to_string())?;
            out.push(ch);
            i += ch.len_utf8();
        }
    }

    Err("Kapatılmamış JSON metni.".to_string())
}

#[cfg(windows)]
fn to_wide_null(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(core::iter::once(0)).collect()
}

#[cfg(windows)]
fn winhttp_get_https(host: &str, path: &str) -> Result<String, String> {
    use core::ffi::c_void;

    type Hinternet = *mut c_void;
    type Bool = i32;

    const WINHTTP_ACCESS_TYPE_DEFAULT_PROXY: u32 = 0;
    const WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY: u32 = 4;
    const INTERNET_DEFAULT_HTTPS_PORT: u16 = 443;
    const WINHTTP_FLAG_SECURE: u32 = 0x0080_0000;
    const WINHTTP_QUERY_STATUS_CODE: u32 = 19;
    const WINHTTP_QUERY_FLAG_NUMBER: u32 = 0x2000_0000;
    const MAX_BODY_BYTES: usize = 512 * 1024;

    #[link(name = "winhttp")]
    extern "system" {
        fn WinHttpOpen(
            psz_agent_w: *const u16,
            dw_access_type: u32,
            psz_proxy_w: *const u16,
            psz_proxy_bypass_w: *const u16,
            dw_flags: u32,
        ) -> Hinternet;
        fn WinHttpSetTimeouts(
            h_internet: Hinternet,
            n_resolve_timeout: i32,
            n_connect_timeout: i32,
            n_send_timeout: i32,
            n_receive_timeout: i32,
        ) -> Bool;
        fn WinHttpConnect(
            h_session: Hinternet,
            pswz_server_name: *const u16,
            n_server_port: u16,
            dw_reserved: u32,
        ) -> Hinternet;
        fn WinHttpOpenRequest(
            h_connect: Hinternet,
            pwsz_verb: *const u16,
            pwsz_object_name: *const u16,
            pwsz_version: *const u16,
            pwsz_referrer: *const u16,
            ppwsz_accept_types: *const *const u16,
            dw_flags: u32,
        ) -> Hinternet;
        fn WinHttpSendRequest(
            h_request: Hinternet,
            lpsz_headers: *const u16,
            dw_headers_length: u32,
            lp_optional: *mut c_void,
            dw_optional_length: u32,
            dw_total_length: u32,
            dw_context: usize,
        ) -> Bool;
        fn WinHttpReceiveResponse(h_request: Hinternet, lp_reserved: *mut c_void) -> Bool;
        fn WinHttpQueryHeaders(
            h_request: Hinternet,
            dw_info_level: u32,
            pwsz_name: *const u16,
            lp_buffer: *mut c_void,
            lpdw_buffer_length: *mut u32,
            lpdw_index: *mut u32,
        ) -> Bool;
        fn WinHttpQueryDataAvailable(h_request: Hinternet, lpdw_number_of_bytes_available: *mut u32) -> Bool;
        fn WinHttpReadData(
            h_request: Hinternet,
            lp_buffer: *mut c_void,
            dw_number_of_bytes_to_read: u32,
            lpdw_number_of_bytes_read: *mut u32,
        ) -> Bool;
        fn WinHttpCloseHandle(h_internet: Hinternet) -> Bool;
    }

    struct HandleGuard(Hinternet);
    impl Drop for HandleGuard {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe {
                    let _ = WinHttpCloseHandle(self.0);
                }
            }
        }
    }

    let agent = to_wide_null("Stoll-Kalip-Editoru/1.0");
    let mut h_session = unsafe {
        WinHttpOpen(
            agent.as_ptr(),
            WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY,
            core::ptr::null(),
            core::ptr::null(),
            0,
        )
    };
    if h_session.is_null() {
        h_session = unsafe {
            WinHttpOpen(
                agent.as_ptr(),
                WINHTTP_ACCESS_TYPE_DEFAULT_PROXY,
                core::ptr::null(),
                core::ptr::null(),
                0,
            )
        };
    }
    if h_session.is_null() {
        return Err("WinHTTP oturumu başlatılamadı.".to_string());
    }
    let session_guard = HandleGuard(h_session);

    unsafe {
        let _ = WinHttpSetTimeouts(session_guard.0, 5000, 5000, 7000, 7000);
    }

    let host_w = to_wide_null(host);
    let h_connect = unsafe {
        WinHttpConnect(
            session_guard.0,
            host_w.as_ptr(),
            INTERNET_DEFAULT_HTTPS_PORT,
            0,
        )
    };
    if h_connect.is_null() {
        return Err("GitHub sunucusuna bağlanılamadı.".to_string());
    }
    let connect_guard = HandleGuard(h_connect);

    let verb_w = to_wide_null("GET");
    let path_w = to_wide_null(path);
    let h_request = unsafe {
        WinHttpOpenRequest(
            connect_guard.0,
            verb_w.as_ptr(),
            path_w.as_ptr(),
            core::ptr::null(),
            core::ptr::null(),
            core::ptr::null(),
            WINHTTP_FLAG_SECURE,
        )
    };
    if h_request.is_null() {
        return Err("HTTPS isteği oluşturulamadı.".to_string());
    }
    let request_guard = HandleGuard(h_request);

    let headers_w = to_wide_null("Accept: application/vnd.github+json\r\n");
    let sent = unsafe {
        WinHttpSendRequest(
            request_guard.0,
            headers_w.as_ptr(),
            u32::MAX,
            core::ptr::null_mut(),
            0,
            0,
            0,
        )
    };
    if sent == 0 {
        return Err("GitHub'a ulaşılamadı (internet bağlantısı yok).".to_string());
    }

    if unsafe { WinHttpReceiveResponse(request_guard.0, core::ptr::null_mut()) } == 0 {
        return Err("GitHub sunucusundan yanıt alınamadı.".to_string());
    }

    let mut status_code: u32 = 0;
    let mut status_size: u32 = core::mem::size_of::<u32>() as u32;
    let qok = unsafe {
        WinHttpQueryHeaders(
            request_guard.0,
            WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
            core::ptr::null(),
            (&mut status_code as *mut u32).cast::<c_void>(),
            &mut status_size,
            core::ptr::null_mut(),
        )
    };
    if qok == 0 {
        return Err("HTTP durum kodu okunamadı.".to_string());
    }
    if status_code != 200 {
        return Err(format!("GitHub yanıt kodu: HTTP {status_code}"));
    }

    let mut body = Vec::new();
    loop {
        let mut avail: u32 = 0;
        if unsafe { WinHttpQueryDataAvailable(request_guard.0, &mut avail) } == 0 {
            return Err("Yanıt verisi okunamadı.".to_string());
        }
        if avail == 0 {
            break;
        }
        let to_read = (avail as usize).min(64 * 1024);
        if body.len() + to_read > MAX_BODY_BYTES {
            return Err("GitHub yanıtı beklenenden büyük.".to_string());
        }
        let mut chunk = vec![0u8; to_read];
        let mut read_bytes: u32 = 0;
        if unsafe {
            WinHttpReadData(
                request_guard.0,
                chunk.as_mut_ptr().cast::<c_void>(),
                to_read as u32,
                &mut read_bytes,
            )
        } == 0
        {
            return Err("Veri akışı okunurken hata oluştu.".to_string());
        }
        if read_bytes == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..read_bytes as usize]);
    }

    String::from_utf8(body).map_err(|_| "GitHub yanıtı geçerli UTF-8 değil.".to_string())
}

#[cfg(not(windows))]
fn winhttp_get_https(_host: &str, _path: &str) -> Result<String, String> {
    Err("Güncelleme denetimi yalnızca Windows üzerinde desteklenir.".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semver_comparison_handles_tags_and_prefixes() {
        assert!(is_newer_version("v1.0.1", "v1.0.0"));
        assert!(is_newer_version("1.1.0", "v1.0.0"));
        assert!(is_newer_version("v2.0.0", "v1.9.9"));
        assert!(!is_newer_version("v1.0.0", "v1.0.0"));
        assert!(!is_newer_version("1.0.0", "v1.0.0"));
        assert!(!is_newer_version("v0.9.9", "v1.0.0"));
        assert!(is_newer_version("v1.0.1-beta", "v1.0.0"));
    }

    #[test]
    fn parses_github_release_json_and_picks_exe_asset() {
        let sample = r#"{
            "url": "https://api.github.com/repos/gokhantr/Stoll-Kalip-Editoru/releases/1",
            "html_url": "https://github.com/gokhantr/Stoll-Kalip-Editoru/releases/tag/v1.0.1",
            "author": {
                "login": "gokhantr",
                "html_url": "https://github.com/gokhantr"
            },
            "tag_name": "v1.0.1",
            "body": "Kavis ve kopyala/yapıştır düzeltmeleri.\nYeni ikon eklendi.",
            "assets": [
                {
                    "name": "checksums.txt",
                    "browser_download_url": "https://github.com/gokhantr/Stoll-Kalip-Editoru/releases/download/v1.0.1/checksums.txt"
                },
                {
                    "name": "Stoll_Kalip_Editoru.exe",
                    "browser_download_url": "https://github.com/gokhantr/Stoll-Kalip-Editoru/releases/download/v1.0.1/Stoll_Kalip_Editoru.exe"
                }
            ]
        }"#;

        let parsed = parse_github_release_json(sample).unwrap();
        assert_eq!(parsed.tag_name, "v1.0.1");
        assert_eq!(
            parsed.html_url,
            "https://github.com/gokhantr/Stoll-Kalip-Editoru/releases/tag/v1.0.1"
        );
        assert_eq!(
            parsed.download_url,
            "https://github.com/gokhantr/Stoll-Kalip-Editoru/releases/download/v1.0.1/Stoll_Kalip_Editoru.exe"
        );
        assert!(parsed.body_notes.contains("Kavis ve kopyala"));

        let summary = format_release_notes_summary(
            "<!-- comment -->\r\n### Yenilikler\r\n- **Kavis** düzeltildi\r\n* İkon eklendi\r\n",
            6,
            200,
        );
        assert_eq!(summary, "Yenilikler\n• Kavis düzeltildi\n• İkon eklendi");
    }
}
