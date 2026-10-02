// Copyright 2026 Daniel Probst <daenuprobst@gmail.com>
// SPDX-License-Identifier: GPL-3.0-only

/// Named symbols (private use area) of the first installed Nerd Font.
pub fn nerd_font_symbols() -> Vec<(char, String)> {
    let mut db = fontdb::Database::new();
    db.load_system_fonts();
    let Some(face) = db.faces().find(|f| {
        f.families
            .iter()
            .any(|(name, _)| name.contains("Nerd Font"))
    }) else {
        return Vec::new();
    };
    db.with_face_data(face.id, |data, index| {
        let Ok(font) = ttf_parser::Face::parse(data, index) else {
            return Vec::new();
        };
        let mut symbols = Vec::new();
        for subtable in font.tables().cmap.iter().flat_map(|c| c.subtables) {
            subtable.codepoints(|cp| {
                let private = (0xE000..=0xF8FF).contains(&cp) || cp >= 0xF0000;
                if let (true, Some(c)) = (private, char::from_u32(cp))
                    && let Some(name) = subtable.glyph_index(cp).and_then(|g| font.glyph_name(g))
                {
                    symbols.push((c, name.to_owned()));
                }
            });
        }
        symbols.sort();
        symbols.dedup();
        symbols
    })
    .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    #[test]
    fn finds_named_symbols() {
        let symbols = super::nerd_font_symbols();
        assert!(symbols.iter().any(|(_, name)| name.contains("firefox")));
    }
}
