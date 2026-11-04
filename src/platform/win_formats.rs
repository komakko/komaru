use crate::core::ingest::HtmlFragment;

const HEADER_LIMIT: usize = 8192;
const BITMAP_FILE_HEADER_LEN: usize = 14;
const INFO_HEADER_LEN: usize = 40;
const COMPRESSION_BITFIELDS: u32 = 3;
const COMPRESSION_ALPHA_BITFIELDS: u32 = 6;

pub fn bmp_from_dib(dib: &[u8]) -> Option<Vec<u8>> {
    let header_len = read_u32(dib, 0)? as usize;
    if header_len < INFO_HEADER_LEN {
        return None;
    }
    let bit_count = u32::from(read_u16(dib, 14)?);
    let compression = read_u32(dib, 16)?;
    let colors_used = read_u32(dib, 32)?;
    let palette_entries = match (colors_used, bit_count) {
        (0, 1..=8) => 1usize << bit_count,
        (used, _) => used as usize,
    };
    let mask_bytes = match (header_len, compression) {
        (INFO_HEADER_LEN, COMPRESSION_BITFIELDS) => 12,
        (INFO_HEADER_LEN, COMPRESSION_ALPHA_BITFIELDS) => 16,
        _ => 0,
    };
    let pixel_offset = header_len
        .checked_add(mask_bytes)?
        .checked_add(palette_entries.checked_mul(4)?)?;
    if pixel_offset > dib.len() {
        return None;
    }
    let file_len = BITMAP_FILE_HEADER_LEN.checked_add(dib.len())?;
    let mut bmp = Vec::with_capacity(file_len);
    bmp.extend_from_slice(b"BM");
    bmp.extend_from_slice(&u32::try_from(file_len).ok()?.to_le_bytes());
    bmp.extend_from_slice(&[0; 4]);
    bmp.extend_from_slice(
        &u32::try_from(BITMAP_FILE_HEADER_LEN + pixel_offset)
            .ok()?
            .to_le_bytes(),
    );
    bmp.extend_from_slice(dib);
    Some(bmp)
}

pub fn html_fragment(clipboard_html: &[u8]) -> Option<HtmlFragment> {
    let header = String::from_utf8_lossy(&clipboard_html[..clipboard_html.len().min(HEADER_LIMIT)]);
    let field =
        |key: &str| -> Option<&str> { header.lines().find_map(|line| line.strip_prefix(key)) };
    let offset = |key: &str| -> Option<usize> { field(key)?.trim().parse().ok() };
    let range = |start_key: &str, end_key: &str| {
        let (start, end) = (offset(start_key)?, offset(end_key)?);
        clipboard_html.get(start..end)
    };
    let markup =
        range("StartFragment:", "EndFragment:").or_else(|| range("StartHTML:", "EndHTML:"))?;
    let markup = String::from_utf8_lossy(markup).into_owned();
    let base_url = field("SourceURL:")
        .map(str::trim)
        .filter(|url| !url.is_empty())
        .map(str::to_owned);
    (!markup.trim().is_empty()).then_some(HtmlFragment { markup, base_url })
}

pub fn utf16_text(bytes: &[u8]) -> String {
    let units = bytes
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .take_while(|&unit| unit != 0);
    char::decode_utf16(units)
        .map(|unit| unit.unwrap_or(char::REPLACEMENT_CHARACTER))
        .collect()
}

fn read_u16(bytes: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(bytes.get(at..at + 2)?.try_into().ok()?))
}

fn read_u32(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info_header(bit_count: u16, compression: u32, colors_used: u32) -> Vec<u8> {
        let mut header = Vec::new();
        header.extend(40u32.to_le_bytes());
        header.extend(2i32.to_le_bytes());
        header.extend(2i32.to_le_bytes());
        header.extend(1u16.to_le_bytes());
        header.extend(bit_count.to_le_bytes());
        header.extend(compression.to_le_bytes());
        header.extend(0u32.to_le_bytes());
        header.extend(0u32.to_le_bytes());
        header.extend(0u32.to_le_bytes());
        header.extend(colors_used.to_le_bytes());
        header.extend(0u32.to_le_bytes());
        header
    }

    fn pixel_offset_of(bmp: &[u8]) -> u32 {
        u32::from_le_bytes(bmp[10..14].try_into().unwrap())
    }

    #[test]
    fn true_colour_dib_gets_a_file_header() {
        let mut dib = info_header(24, 0, 0);
        dib.extend([0; 16]);
        let bmp = bmp_from_dib(&dib).unwrap();
        assert_eq!(&bmp[..2], b"BM");
        assert_eq!(pixel_offset_of(&bmp), 54);
        assert_eq!(bmp.len(), dib.len() + 14);
        assert_eq!(&bmp[14..], &dib[..]);
    }

    #[test]
    fn palette_and_bitfield_masks_move_the_pixel_offset() {
        let mut indexed = info_header(8, 0, 0);
        indexed.extend([0; 1024 + 4]);
        assert_eq!(
            pixel_offset_of(&bmp_from_dib(&indexed).unwrap()),
            14 + 40 + 1024
        );

        let mut masked = info_header(32, COMPRESSION_BITFIELDS, 0);
        masked.extend([0; 12 + 16]);
        assert_eq!(
            pixel_offset_of(&bmp_from_dib(&masked).unwrap()),
            14 + 40 + 12
        );

        let mut explicit = info_header(4, 0, 3);
        explicit.extend([0; 12 + 4]);
        assert_eq!(
            pixel_offset_of(&bmp_from_dib(&explicit).unwrap()),
            14 + 40 + 12
        );
    }

    #[test]
    fn truncated_or_foreign_dibs_are_rejected() {
        assert!(bmp_from_dib(&[]).is_none());
        assert!(bmp_from_dib(&[0; 20]).is_none());
        let mut indexed = info_header(8, 0, 0);
        indexed.extend([0; 16]);
        assert!(bmp_from_dib(&indexed).is_none());
        let mut core_header = 12u32.to_le_bytes().to_vec();
        core_header.extend([0; 40]);
        assert!(bmp_from_dib(&core_header).is_none());
    }

    #[test]
    fn converted_dib_decodes_as_an_image() {
        let mut dib = info_header(24, 0, 0);
        dib.extend([10, 20, 30, 40, 50, 60, 0, 0, 70, 80, 90, 1, 2, 3, 0, 0]);
        let bmp = bmp_from_dib(&dib).unwrap();
        let image = image::load_from_memory(&bmp).unwrap();
        assert_eq!((image.width(), image.height()), (2, 2));
    }

    fn clipboard_html(fragment: &str) -> Vec<u8> {
        clipboard_html_from(None, fragment)
    }

    fn clipboard_html_from(source: Option<&str>, fragment: &str) -> Vec<u8> {
        let prefix = "<html><body><!--StartFragment-->";
        let suffix = "<!--EndFragment--></body></html>";
        let header_len = 256;
        let start_html = header_len;
        let start_fragment = header_len + prefix.len();
        let end_fragment = start_fragment + fragment.len();
        let end_html = end_fragment + suffix.len();
        let source_line = source
            .map(|url| format!("SourceURL:{url}\r\n"))
            .unwrap_or_default();
        let header = format!(
            "Version:0.9\r\nStartHTML:{start_html:010}\r\nEndHTML:{end_html:010}\r\nStartFragment:{start_fragment:010}\r\nEndFragment:{end_fragment:010}\r\n{source_line}"
        );
        let padded = format!("{header:<width$}", width = header_len);
        format!("{padded}{prefix}{fragment}{suffix}").into_bytes()
    }

    #[test]
    fn html_fragment_is_cut_by_header_offsets() {
        let data = clipboard_html("<img src=\"https://a.test/i.png\">");
        let fragment = html_fragment(&data).unwrap();
        assert_eq!(fragment.markup, "<img src=\"https://a.test/i.png\">");
        assert_eq!(fragment.base_url, None);
    }

    #[test]
    fn source_url_becomes_the_base_url() {
        let data = clipboard_html_from(Some("https://a.test/page.html"), "<img src=\"a.png\">");
        let fragment = html_fragment(&data).unwrap();
        assert_eq!(fragment.markup, "<img src=\"a.png\">");
        assert_eq!(
            fragment.base_url.as_deref(),
            Some("https://a.test/page.html")
        );
    }

    #[test]
    fn html_without_usable_header_is_rejected() {
        assert!(html_fragment(b"<img src=x>").is_none());
        assert!(html_fragment(b"").is_none());
        assert!(html_fragment(b"StartFragment:0000009999\r\nEndFragment:0000009999\r\n").is_none());
    }

    #[test]
    fn utf16_text_stops_at_the_terminator() {
        let bytes: Vec<u8> = "https://a.test/x.png\0junk"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect();
        assert_eq!(utf16_text(&bytes), "https://a.test/x.png");
        assert_eq!(utf16_text(&[]), "");
    }
}
