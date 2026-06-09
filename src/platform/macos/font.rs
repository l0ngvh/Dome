use anyhow::Context;
use objc2_core_foundation::{CFString, CFURL};
use objc2_core_text::{CTFontDescriptor, kCTFontURLAttribute};

pub(crate) fn resolve_system_font(family: &str) -> anyhow::Result<Vec<u8>> {
    let cf_name = CFString::from_str(family);
    let descriptor = unsafe { CTFontDescriptor::with_name_and_size(&cf_name, 0.0) };
    let url_attr_key = unsafe { kCTFontURLAttribute };
    let url_value = unsafe { descriptor.attribute(url_attr_key) }
        .ok_or_else(|| anyhow::anyhow!("no font matches family '{family}'"))?;
    let url: &CFURL = url_value
        .downcast_ref()
        .ok_or_else(|| anyhow::anyhow!("kCTFontURLAttribute did not return a CFURL"))?;
    let path = url
        .to_file_path()
        .ok_or_else(|| anyhow::anyhow!("font URL is not a file path"))?;
    let bytes =
        std::fs::read(&path).with_context(|| format!("reading font file {}", path.display()))?;
    Ok(bytes)
}
