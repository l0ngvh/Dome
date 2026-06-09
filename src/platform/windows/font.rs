use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateFontIndirectW, DeleteDC, DeleteObject, GDI_ERROR, GetFontData, HDC,
    HFONT, LOGFONTW, SelectObject,
};

pub(crate) fn resolve_system_font(family: &str) -> anyhow::Result<Vec<u8>> {
    let hdc = unsafe { CreateCompatibleDC(None) };
    if hdc.is_invalid() {
        anyhow::bail!("CreateCompatibleDC failed");
    }
    let _dc = DcGuard(hdc);

    let mut lf = LOGFONTW::default();
    let face: Vec<u16> = family.encode_utf16().take(31).collect();
    lf.lfFaceName[..face.len()].copy_from_slice(&face);

    let hfont = unsafe { CreateFontIndirectW(&lf) };
    if hfont.is_invalid() {
        anyhow::bail!("CreateFontIndirectW failed for '{family}'");
    }
    let prev = unsafe { SelectObject(hdc, hfont.into()) };
    let _font = FontGuard { hdc, hfont, prev };

    const TTCF: u32 = 0x66637474;
    let ttc_size = unsafe { GetFontData(hdc, TTCF, 0, None, 0) };
    let (table, expected) = if ttc_size != GDI_ERROR as u32 {
        (TTCF, ttc_size)
    } else {
        let size = unsafe { GetFontData(hdc, 0, 0, None, 0) };
        if size == GDI_ERROR as u32 {
            anyhow::bail!("GetFontData refused '{family}' (non-TrueType or non-embeddable)");
        }
        (0, size)
    };

    let mut buf = vec![0u8; expected as usize];
    let n = unsafe { GetFontData(hdc, table, 0, Some(buf.as_mut_ptr().cast()), expected) };
    if n != expected {
        anyhow::bail!("GetFontData short read for '{family}'");
    }
    Ok(buf)
}

struct DcGuard(HDC);

impl Drop for DcGuard {
    fn drop(&mut self) {
        unsafe { DeleteDC(self.0) }.ok().ok();
    }
}

struct FontGuard {
    hdc: HDC,
    hfont: HFONT,
    // SelectObject(hdc, prev) must run before DeleteObject(hfont). Deleting
    // an HFONT still selected into a DC silently fails and leaks the handle.
    prev: windows::Win32::Graphics::Gdi::HGDIOBJ,
}

impl Drop for FontGuard {
    fn drop(&mut self) {
        unsafe { SelectObject(self.hdc, self.prev) };
        unsafe { DeleteObject(self.hfont.into()) }.ok().ok();
    }
}
