use crate::{engine, models::*, workbooks::*};
use bigdecimal::BigDecimal;
use chrono::{Datelike, NaiveDate};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Write,
    path::{Path, PathBuf},
};

/// Remove grouping tokens, not scaling commas, literals, fractional separators,
/// conditions, currency labels, percentages or scientific-notation precision.
pub fn remove_grouping(format: &str) -> String {
    let tokens = format_tokens(format);
    let digit = |s: &str| matches!(s, "#" | "0" | "?");
    let mut output = String::new();
    for (i, token) in tokens.iter().enumerate() {
        let grouped = matches!(token.as_str(), "," | " " | "\u{a0}" | "\u{202f}")
            && i > 0
            && i + 1 < tokens.len()
            && digit(&tokens[i - 1])
            && digit(&tokens[i + 1]);
        let fraction = token != ","
            && tokens[i + 1..].iter().take_while(|t| digit(t)).count() > 0
            && tokens[i + 1..]
                .iter()
                .skip_while(|t| digit(t))
                .next()
                .is_some_and(|t| t == "/");
        if !grouped || fraction {
            output.push_str(token);
        }
    }
    output
}

fn numeric_format(raw: &str, previous: &str) -> Result<String> {
    let n = number(raw)?.normalized();
    let format = remove_grouping(previous);
    if format == "General" || format == "@" || date_format(&format) {
        return Ok("General".into());
    }
    let mut result = String::new();
    let tokens = format_tokens(&format);
    for (index, section) in tokens.split(|t| t == ";").enumerate() {
        if index > 0 {
            result.push(';');
        }
        let mut section = section.to_vec();
        let digit = |t: &str| matches!(t, "0" | "#" | "?");
        let scientific = section.iter().position(|t| t.eq_ignore_ascii_case("e"));
        if section.iter().any(|t| t == "/") || !section.iter().any(|t| digit(t)) {
            result.push_str(&section.concat());
            continue;
        }
        let end = scientific.unwrap_or(section.len());
        let last = (0..end).rev().find(|i| digit(&section[*i])).unwrap_or(0);
        let dot = section[..=last].iter().position(|t| t == ".");
        let existing = dot.map_or(0, |d| {
            section[d + 1..=last].iter().filter(|t| digit(t)).count()
        });
        let percent = section.iter().filter(|t| *t == "%").count() as i64;
        let scaling = section[last + 1..end]
            .iter()
            .take_while(|t| *t == ",")
            .count() as i64;
        let (coefficient, exponent) = n.as_bigint_and_exponent();
        let needed = if scientific.is_some() {
            coefficient
                .to_string()
                .trim_start_matches('-')
                .len()
                .saturating_sub(1) as i64
        } else {
            (exponent - percent * 2 + scaling * 3).max(0)
        };
        if needed > 30 {
            // Replacing a whole custom format with scientific notation would drop
            // negative sections, conditions and quoted units. Refuse instead.
            return Err(error("Le format numerique exige plus de 30 decimales pour conserver la precision. Choisissez un format scientifique ou General dans une copie source, puis relancez l'analyse."));
        }
        if needed as usize > existing {
            let extra = "#".repeat(needed as usize - existing);
            let extra = if dot.is_none() {
                format!(".{extra}")
            } else {
                extra
            };
            section.insert(last + 1, extra);
        }
        result.push_str(&section.concat());
    }
    if result.chars().count() > 255 && result != format {
        return Err(error(
            "Le format numerique precis depasse la limite Excel de 255 caracteres.",
        ));
    }
    Ok(result)
}

pub(crate) fn validate_cell(value: &CellValue) -> Result<()> {
    match value.kind.as_str() {
        "empty" if value.value.is_empty() => {}
        "text" => {
            if value.value.encode_utf16().count() > 32_767 {
                return Err(error("Texte trop long pour une cellule Excel."));
            }
        }
        "integer" | "decimal" => {
            let n = number(&value.value)?;
            if value.kind == "integer" && n.normalized().as_bigint_and_exponent().1 > 0 {
                return Err(error("Un entier ne peut pas contenir de fraction."));
            }
        }
        "date" => {
            let date = NaiveDate::parse_from_str(&value.value, "%Y-%m-%d")
                .map_err(|_| error("Date ISO invalide."))?;
            if !(1900..=9999).contains(&date.year())
                || date.format("%Y-%m-%d").to_string() != value.value
            {
                return Err(error("Date hors limites Excel."));
            }
        }
        "boolean" if matches!(value.value.as_str(), "true" | "false") => {}
        "formula" | "error" => {}
        _ => return Err(error("Valeur de cellule typee invalide.")),
    }
    Ok(())
}

fn export_precision(value: &CellValue) -> Result<()> {
    validate_cell(value)?;
    if matches!(value.kind.as_str(), "formula" | "error") {
        return Err(error(
            "Une correction ne peut pas introduire de formule ou d'erreur Excel.",
        ));
    }
    if matches!(value.kind.as_str(), "integer" | "decimal") {
        let n = number(&value.value)?.normalized();
        let (coefficient, exponent) = n.as_bigint_and_exponent();
        let digits = coefficient.to_string().trim_start_matches('-').len();
        let magnitude = digits as i64 - exponent - 1;
        if n != BigDecimal::from(0) && (digits > 15 || !(-307..=307).contains(&magnitude)) {
            return Err(error("Precision non representable de facon fiable dans Excel (15 chiffres significatifs). Conservez la valeur en texte."));
        }
    }
    Ok(())
}

fn normalize_inline(bytes: &[u8], job: &JobControl) -> Result<Vec<u8>> {
    let xml = Xml::parse(bytes, job)?;
    let mut changes = Vec::new();
    for node in &xml.nodes {
        let mut updates = Vec::new();
        if node.local() == "alignment" && node.attr("textRotation").is_some_and(|v| v != "0") {
            updates.push(("textRotation", Some("0".into())));
        }
        if node.local() == "numFmt" {
            if let Some(code) = node.attr("formatCode") {
                let normalized = remove_grouping(code);
                if normalized != code {
                    updates.push(("formatCode", Some(normalized)));
                }
            } else if let Some(id) = node.attr("numFmtId") {
                let id: u32 = id
                    .parse()
                    .map_err(|_| error("Format conditionnel invalide."))?;
                let code = builtin_format(id)
                    .ok_or_else(|| error("Format conditionnel non pris en charge."))?;
                updates.push(("formatCode", Some(remove_grouping(code))));
            }
        }
        if !updates.is_empty() {
            changes.push((
                node.start,
                node.open_end,
                node.tag(&updates, node.empty).into_bytes(),
            ));
        }
    }
    patch(bytes, changes)
}

fn ensure_styles(package: &mut Package, job: &JobControl) -> Result<()> {
    if package.parts.contains_key("xl/styles.xml") {
        return Ok(());
    }
    let styles = format!("<styleSheet xmlns=\"{MAIN_NS}\"><fonts count=\"1\"><font/></fonts><fills count=\"2\"><fill><patternFill patternType=\"none\"/></fill><fill><patternFill patternType=\"gray125\"/></fill></fills><borders count=\"1\"><border/></borders><cellStyleXfs count=\"1\"><xf numFmtId=\"0\" fontId=\"0\" fillId=\"0\" borderId=\"0\"/></cellStyleXfs><cellXfs count=\"1\"><xf numFmtId=\"0\" fontId=\"0\" fillId=\"0\" borderId=\"0\" xfId=\"0\"/></cellXfs><cellStyles count=\"1\"><cellStyle name=\"Normal\" xfId=\"0\" builtinId=\"0\"/></cellStyles></styleSheet>");
    package
        .parts
        .insert("xl/styles.xml".into(), styles.into_bytes());
    add_relation(package, "styles.xml", "styles", job)?;
    add_content_type(
        package,
        "/xl/styles.xml",
        "application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml",
        job,
    )
}

fn append_format(bytes: &[u8], id: u32, code: &str, job: &JobControl) -> Result<Vec<u8>> {
    let xml = Xml::parse(bytes, job)?;
    let root = &xml.nodes[xml.root];
    let child_name = root.qualified("numFmt");
    let new = format!(
        "<{child_name} numFmtId=\"{id}\" formatCode=\"{}\"/>",
        escape(code)
    );
    if let Some(parent) = xml.child(xml.root, "numFmts") {
        let count = xml.children(parent, "numFmt").count() + 1;
        append_children(
            bytes,
            &xml.nodes[parent],
            &new,
            &[("count", Some(count.to_string()))],
        )
    } else {
        let name = root.qualified("numFmts");
        patch(
            bytes,
            vec![(
                root.open_end,
                root.open_end,
                format!("<{name} count=\"1\">{new}</{name}>").into_bytes(),
            )],
        )
    }
}

fn harmonize_styles(bytes: &[u8], job: &JobControl) -> Result<Vec<u8>> {
    let mut bytes = normalize_inline(bytes, job)?;
    let xml = Xml::parse(&bytes, job)?;
    let mut max_id = 163u32;
    for node in &xml.nodes {
        if node.local() == "numFmt" {
            let id: u32 = node
                .required("numFmtId")?
                .parse()
                .map_err(|_| error("Format numerique invalide."))?;
            max_id = max_id.max(id);
        }
    }
    let mut mapped = BTreeMap::new();
    let mut changes = Vec::new();
    for node in &xml.nodes {
        if node.local() != "xf" {
            continue;
        }
        let id: u32 = node
            .attr("numFmtId")
            .unwrap_or("0")
            .parse()
            .map_err(|_| error("Style Excel invalide."))?;
        if let Some(code) = builtin_format(id) {
            let new = remove_grouping(code);
            if new == code {
                continue;
            }
            let replacement = if let Some(value) = mapped.get(&id) {
                *value
            } else {
                max_id = max_id
                    .checked_add(1)
                    .ok_or_else(|| error("Trop de formats Excel."))?;
                mapped.insert(id, max_id);
                max_id
            };
            changes.push((
                node.start,
                node.open_end,
                node.tag(&[("numFmtId", Some(replacement.to_string()))], node.empty)
                    .into_bytes(),
            ));
        }
    }
    bytes = patch(&bytes, changes)?;
    for (old, new) in mapped {
        bytes = append_format(
            &bytes,
            new,
            &remove_grouping(builtin_format(old).unwrap_or("General")),
            job,
        )?;
    }
    Ok(bytes)
}

struct StyleEditor {
    bytes: Vec<u8>,
    formats: Vec<String>,
    cache: BTreeMap<(usize, String), usize>,
    next_id: u32,
}
impl StyleEditor {
    fn new(package: &Package, job: &JobControl) -> Result<Self> {
        let bytes = package.get("xl/styles.xml")?.to_vec();
        let xml = Xml::parse(&bytes, job)?;
        let mut next_id = 164;
        for node in &xml.nodes {
            if node.local() == "numFmt" {
                let id: u32 = node
                    .required("numFmtId")?
                    .parse()
                    .map_err(|_| error("Format numerique invalide."))?;
                next_id = next_id.max(
                    id.checked_add(1)
                        .ok_or_else(|| error("Format Excel hors limites."))?,
                );
            }
        }
        Ok(Self {
            bytes,
            formats: Styles::read(package, job)?.formats,
            cache: BTreeMap::new(),
            next_id,
        })
    }
    fn with_format(&mut self, original: usize, code: &str, job: &JobControl) -> Result<usize> {
        if self.formats.get(original).is_some_and(|v| v == code) {
            return Ok(original);
        }
        if let Some(id) = self.cache.get(&(original, code.to_string())) {
            return Ok(*id);
        }
        if self.formats.len() >= 64_000 {
            return Err(error("Limite des styles Excel atteinte."));
        }
        let format_id = self.next_id;
        self.next_id = self
            .next_id
            .checked_add(1)
            .ok_or_else(|| error("Trop de formats Excel."))?;
        self.bytes = append_format(&self.bytes, format_id, code, job)?;
        let xml = Xml::parse(&self.bytes, job)?;
        let parent = xml
            .child(xml.root, "cellXfs")
            .ok_or_else(|| error("Table des styles absente."))?;
        let children: Vec<_> = xml.children(parent, "xf").collect();
        let id = *children
            .get(original)
            .ok_or_else(|| error("Style d'origine absent."))?;
        let node = &xml.nodes[id];
        let mut new = node.tag(
            &[
                ("numFmtId", Some(format_id.to_string())),
                ("applyNumberFormat", Some("1".into())),
            ],
            node.empty,
        );
        if !node.empty {
            new.push_str(
                std::str::from_utf8(&self.bytes[node.open_end..node.end])
                    .map_err(|e| io_error("Style XML invalide", e))?,
            );
        }
        let index = children.len();
        self.bytes = append_children(
            &self.bytes,
            &xml.nodes[parent],
            &new,
            &[("count", Some((index + 1).to_string()))],
        )?;
        self.formats.push(code.to_owned());
        self.cache.insert((original, code.to_owned()), index);
        Ok(index)
    }
}

fn serialized_cell(
    node: Option<&Node>,
    row: u32,
    column: u32,
    value: &CellValue,
    style: usize,
    parent: &Node,
) -> Result<String> {
    let name = node.map_or_else(|| parent.qualified("c"), |n| n.name.clone());
    let kind = match value.kind.as_str() {
        "text" => Some("inlineStr"),
        "boolean" => Some("b"),
        "date" => Some("d"),
        "integer" | "decimal" => Some("n"),
        "empty" => None,
        _ => return Err(error("Type de correction Excel interdit.")),
    };
    let updates = [
        ("r", Some(address(row, column))),
        ("s", Some(style.to_string())),
        ("t", kind.map(str::to_owned)),
    ];
    let start = if let Some(node) = node {
        node.tag(&updates, value.is_empty())
    } else {
        let mut text = format!("<{name} r=\"{}\" s=\"{style}\"", address(row, column));
        if let Some(kind) = kind {
            text.push_str(&format!(" t=\"{kind}\""));
        }
        text.push_str(if value.is_empty() { "/>" } else { ">" });
        text
    };
    if value.is_empty() {
        return Ok(start);
    }
    let qualified = |local| parent.qualified(local);
    let contents = if value.kind == "text" {
        let is = qualified("is");
        let t = qualified("t");
        format!(
            "<{is}><{t} xml:space=\"preserve\">{}</{t}></{is}>",
            escape(&encode_excel_text(&value.value))
        )
    } else {
        let v = qualified("v");
        let raw = if value.kind == "boolean" {
            if value.value == "true" {
                "1"
            } else {
                "0"
            }
        } else {
            &value.value
        };
        format!("<{v}>{}</{v}>", escape(raw))
    };
    Ok(format!("{start}{contents}</{name}>"))
}

fn effective_style(xml: &Xml, cell: Option<&Node>, row: &Node, column: u32) -> Result<usize> {
    if let Some(style) = cell.and_then(|c| c.attr("s")).or_else(|| row.attr("s")) {
        return style.parse().map_err(|_| error("Style Excel invalide."));
    }
    if let Some(cols) = xml.child(xml.root, "cols") {
        for id in xml.children(cols, "col") {
            let col = &xml.nodes[id];
            let min: u32 = col
                .required("min")?
                .parse()
                .map_err(|_| error("Dimension invalide."))?;
            let max: u32 = col
                .required("max")?
                .parse()
                .map_err(|_| error("Dimension invalide."))?;
            if (min..=max).contains(&column) {
                return col
                    .attr("style")
                    .unwrap_or("0")
                    .parse()
                    .map_err(|_| error("Style Excel invalide."));
            }
        }
    }
    Ok(0)
}

fn write_sheet(
    bytes: &[u8],
    source: &SheetData,
    changes: &BTreeMap<(u32, u32), CellValue>,
    config: &ProjectConfig,
    styles: &mut StyleEditor,
    job: &JobControl,
) -> Result<Vec<u8>> {
    let xml = Xml::parse(bytes, job)?;
    let sheet_data = xml
        .child(xml.root, "sheetData")
        .ok_or_else(|| error("Donnees de feuille absentes."))?;
    let data_node = &xml.nodes[sheet_data];
    let mut pending = changes.clone();
    let mut patches = Vec::new();
    let mut rows = BTreeMap::new();
    for row_id in xml.children(sheet_data, "row") {
        let row_node = &xml.nodes[row_id];
        let row: u32 = row_node
            .required("r")?
            .parse()
            .map_err(|_| error("Ligne Excel invalide."))?;
        rows.insert(row, row_id);
        let mut cells = BTreeMap::new();
        for cell_id in xml.children(row_id, "c") {
            let cell = &xml.nodes[cell_id];
            let (_, col) = coordinate(cell.required("r")?)?;
            cells.insert(col, cell_id);
        }
        let columns: BTreeSet<_> = cells
            .keys()
            .copied()
            .chain(
                changes
                    .range((row, 0)..=(row, u32::MAX))
                    .map(|((_, col), _)| *col),
            )
            .collect();
        let mut insertions: BTreeMap<usize, String> = BTreeMap::new();
        for col in columns {
            job.check()?;
            let cell = cells.get(&col).map(|id| &xml.nodes[*id]);
            let changed = pending.remove(&(row, col));
            let value = changed
                .as_ref()
                .cloned()
                .unwrap_or_else(|| source.cell(row, col));
            let original_style = effective_style(&xml, cell, row_node, col)?;
            let previous = styles
                .formats
                .get(original_style)
                .ok_or_else(|| error("Style Excel introuvable."))?
                .clone();
            if changed.is_some() {
                let code = match value.kind.as_str() {
                    "text" => "@".to_string(),
                    "date" => config
                        .mappings
                        .iter()
                        .find(|m| m.column == col)
                        .and_then(|m| {
                            config
                                .fields
                                .iter()
                                .find(|f| Some(&f.code) == m.field_code.as_ref())
                        })
                        .map(|f| {
                            f.date_format
                                .replace("AAAA", "yyyy")
                                .replace("YYYY", "yyyy")
                                .replace("JJ", "dd")
                                .replace("DD", "dd")
                                .replace("MM", "mm")
                        })
                        .unwrap_or_else(|| "yyyy-mm-dd".into()),
                    "integer" | "decimal" => numeric_format(&value.value, &previous)?,
                    _ => previous.clone(),
                };
                let style = styles.with_format(original_style, &code, job)?;
                let new = serialized_cell(cell, row, col, &value, style, row_node)?;
                if let Some(cell) = cell {
                    patches.push((cell.start, cell.end, new.into_bytes()));
                } else {
                    let position = cells
                        .range(col + 1..)
                        .next()
                        .map_or(row_node.close_start, |(_, id)| xml.nodes[*id].start);
                    insertions.entry(position).or_default().push_str(&new);
                }
            } else if matches!(value.kind.as_str(), "integer" | "decimal")
                && (!date_format(&previous)
                    || number(&value.value).is_ok_and(|n| {
                        n < BigDecimal::from(0)
                            || n > BigDecimal::from(2_958_465)
                            || (n.normalized().as_bigint_and_exponent().1 <= 0
                                && n > BigDecimal::from(60))
                    }))
            {
                let code = numeric_format(&value.value, &previous)?;
                let style = styles.with_format(original_style, &code, job)?;
                if style != original_style {
                    if let Some(cell) = cell {
                        patches.push((
                            cell.start,
                            cell.open_end,
                            cell.tag(&[("s", Some(style.to_string()))], cell.empty)
                                .into_bytes(),
                        ));
                    }
                }
            }
        }
        if row_node.empty && !insertions.is_empty() {
            patches.push((
                row_node.start,
                row_node.end,
                format!(
                    "{}{}</{}>",
                    row_node.tag(&[], false),
                    insertions.into_values().collect::<String>(),
                    row_node.name
                )
                .into_bytes(),
            ));
        } else {
            for (position, text) in insertions {
                patches.push((position, position, text.into_bytes()));
            }
        }
    }
    let mut new_rows: BTreeMap<u32, String> = BTreeMap::new();
    for ((row, col), value) in pending {
        let code = match value.kind.as_str() {
            "text" => "@",
            "date" => "yyyy-mm-dd",
            _ => "General",
        };
        let style = styles.with_format(0, code, job)?;
        new_rows
            .entry(row)
            .or_default()
            .push_str(&serialized_cell(None, row, col, &value, style, data_node)?);
    }
    let mut insertions: BTreeMap<usize, String> = BTreeMap::new();
    for (row, content) in new_rows {
        let name = data_node.qualified("row");
        let content = format!("<{name} r=\"{row}\">{content}</{name}>");
        let position = rows
            .range(row + 1..)
            .next()
            .map_or(data_node.close_start, |(_, id)| xml.nodes[*id].start);
        insertions.entry(position).or_default().push_str(&content);
    }
    if data_node.empty && !insertions.is_empty() {
        patches.push((
            data_node.start,
            data_node.end,
            format!(
                "{}{}</{}>",
                data_node.tag(&[], false),
                insertions.into_values().collect::<String>(),
                data_node.name
            )
            .into_bytes(),
        ));
    } else {
        for (position, content) in insertions {
            patches.push((position, position, content.into_bytes()));
        }
    }
    patch(bytes, patches)
}

fn add_relation(
    package: &mut Package,
    target: &str,
    kind: &str,
    job: &JobControl,
) -> Result<String> {
    let path = "xl/_rels/workbook.xml.rels";
    let bytes = package.get(path)?;
    let xml = Xml::parse(bytes, job)?;
    let ids: BTreeSet<_> = xml
        .children(xml.root, "Relationship")
        .filter_map(|i| xml.nodes[i].attr("Id"))
        .collect();
    let mut index = 1u32;
    while ids.contains(format!("rIdFDX{index}").as_str()) {
        index += 1;
    }
    let id = format!("rIdFDX{index}");
    let root = &xml.nodes[xml.root];
    let name = root.qualified("Relationship");
    let child = format!(
        "<{name} Id=\"{id}\" Type=\"{REL_NS}/{kind}\" Target=\"{}\"/>",
        escape(target)
    );
    let output = append_children(bytes, root, &child, &[])?;
    package.parts.insert(path.into(), output);
    Ok(id)
}

fn add_content_type(package: &mut Package, path: &str, kind: &str, job: &JobControl) -> Result<()> {
    let bytes = package.get("[Content_Types].xml")?;
    let xml = Xml::parse(bytes, job)?;
    if xml
        .children(xml.root, "Override")
        .any(|i| xml.nodes[i].attr("PartName") == Some(path))
    {
        return Ok(());
    }
    let root = &xml.nodes[xml.root];
    let name = root.qualified("Override");
    let child = format!(
        "<{name} PartName=\"{}\" ContentType=\"{}\"/>",
        escape(path),
        escape(kind)
    );
    let output = append_children(bytes, root, &child, &[])?;
    package.parts.insert("[Content_Types].xml".into(), output);
    Ok(())
}

fn report_sheet(rows: &[Vec<CellValue>], job: &JobControl) -> Result<Vec<u8>> {
    if rows.len() > 1_048_576 {
        return Err(error(
            "Le rapport depasse le nombre de lignes Excel autorise.",
        ));
    }
    let columns = rows.iter().map(Vec::len).max().unwrap_or(1) as u32;
    let end = address(rows.len().max(1) as u32, columns.max(1));
    let mut xml = format!("<worksheet xmlns=\"{MAIN_NS}\"><dimension ref=\"A1:{end}\"/><sheetViews><sheetView workbookViewId=\"0\"><pane ySplit=\"1\" topLeftCell=\"A2\" activePane=\"bottomLeft\" state=\"frozen\"/></sheetView></sheetViews><cols><col min=\"1\" max=\"{columns}\" width=\"24\" customWidth=\"1\"/></cols><sheetData>");
    for (r, values) in rows.iter().enumerate() {
        job.check()?;
        let row = r as u32 + 1;
        xml.push_str(&format!("<row r=\"{row}\">"));
        for (c, value) in values.iter().enumerate() {
            validate_cell(value)?;
            let coordinate = address(row, c as u32 + 1);
            if matches!(value.kind.as_str(), "integer" | "decimal") {
                xml.push_str(&format!(
                    "<c r=\"{coordinate}\" t=\"n\"><v>{}</v></c>",
                    escape(&value.value)
                ));
            } else {
                xml.push_str(&format!("<c r=\"{coordinate}\" t=\"inlineStr\"><is><t xml:space=\"preserve\">{}</t></is></c>", escape(&encode_excel_text(&value.value))));
            }
        }
        xml.push_str("</row>");
    }
    xml.push_str(&format!(
        "</sheetData><autoFilter ref=\"A1:{end}\"/></worksheet>"
    ));
    Ok(xml.into_bytes())
}

fn report_rows(code: &str, config: &ProjectConfig, result: &AnalysisResult) -> Vec<Vec<CellValue>> {
    let text = |s: &str| CellValue::text(s);
    let count = |n: usize| CellValue::typed("integer", n.to_string());
    let mut rows = Vec::new();
    match code {
        "control" => {
            rows.push(
                [
                    "Code",
                    "Colonne",
                    "Libelle",
                    "Stockage",
                    "Role",
                    "Taille max",
                    "Lignes analysees",
                    "Renseignees",
                    "Vides",
                    "Cellules non conformes avant",
                    "Cellules modifiees",
                    "Cellules videes",
                    "Anomalies restantes",
                    "Longueur max",
                ]
                .into_iter()
                .map(text)
                .collect(),
            );
            for field in &config.fields {
                let Some(mapping) = config
                    .mappings
                    .iter()
                    .find(|m| m.field_code.as_deref() == Some(&field.code))
                else {
                    continue;
                };
                let filled = result.filled.get(&field.code).copied().unwrap_or(0);
                rows.push(vec![
                    text(&field.code),
                    count(mapping.column as usize),
                    text(&field.label),
                    text(&field.dtype),
                    text(&field.role),
                    field.max_length.map(count).unwrap_or_else(CellValue::empty),
                    count(result.row_count as usize),
                    count(filled),
                    count((result.row_count as usize).saturating_sub(filled)),
                    count(result.counts_before.get(&field.code).copied().unwrap_or(0)),
                    count(
                        result
                            .changes
                            .iter()
                            .filter(|c| c.field_code == field.code)
                            .count(),
                    ),
                    count(
                        result
                            .changes
                            .iter()
                            .filter(|c| c.field_code == field.code && c.after.is_empty())
                            .count(),
                    ),
                    count(
                        result
                            .findings
                            .iter()
                            .filter(|f| f.field_code == field.code)
                            .count(),
                    ),
                    count(
                        result
                            .maximum_lengths
                            .get(&field.code)
                            .copied()
                            .unwrap_or(0),
                    ),
                ]);
            }
        }
        "original" => {
            rows.push(
                ["Feuille", "Ligne", "Colonne", "Valeur source"]
                    .into_iter()
                    .map(text)
                    .collect(),
            );
            let mut seen = BTreeSet::new();
            for change in result.originals.iter().chain(&result.changes) {
                if seen.insert((change.row, change.column)) {
                    rows.push(vec![
                        text(&config.sheet),
                        count(change.row as usize),
                        count(change.column as usize),
                        text(&change.before.value),
                    ]);
                }
            }
        }
        "verify" => {
            rows.push(
                [
                    "Feuille",
                    "Ligne",
                    "Colonne",
                    "Code",
                    "Valeur source",
                    "Valeur actuelle",
                    "Categorie",
                    "Probleme",
                    "Proposition",
                ]
                .into_iter()
                .map(text)
                .collect(),
            );
            for finding in &result.findings {
                rows.push(vec![
                    text(&config.sheet),
                    count(finding.row as usize),
                    count(finding.column as usize),
                    text(&finding.field_code),
                    text(&finding.original.value),
                    text(&finding.current.value),
                    text(&finding.kind),
                    text(&finding.message),
                    text(finding.proposed.as_ref().map_or("", |v| &v.value)),
                ]);
            }
        }
        _ => {}
    }
    rows
}

fn add_report(
    package: &mut Package,
    base: &str,
    rows: &[Vec<CellValue>],
    config: &ProjectConfig,
    job: &JobControl,
) -> Result<String> {
    let parts = sheet_parts(package, job)?;
    let names: BTreeSet<_> = parts.iter().map(|s| s.name.to_lowercase()).collect();
    let mut name = base.to_string();
    let existing = parts.iter().find(|s| {
        s.name.eq_ignore_ascii_case(base) || s.name.to_lowercase() == base.to_lowercase()
    });
    if let Some(existing) = existing {
        if config.report_collision == "replace" {
            if existing.name == config.sheet {
                return Err(error(
                    "La feuille de donnees ne peut pas etre remplacee par un rapport.",
                ));
            }
            package
                .parts
                .insert(existing.path.clone(), report_sheet(rows, job)?);
            return Ok(existing.name.clone());
        }
        let mut index = 2;
        while names.contains(&name.to_lowercase()) {
            name = format!("{base} ({index})");
            index += 1;
        }
    }
    let mut index = 1;
    let mut path = format!("xl/worksheets/fdx-report-{index}.xml");
    while package.parts.contains_key(&path) {
        index += 1;
        path = format!("xl/worksheets/fdx-report-{index}.xml");
    }
    let rid = add_relation(
        package,
        path.strip_prefix("xl/").unwrap_or(&path),
        "worksheet",
        job,
    )?;
    add_content_type(
        package,
        &format!("/{path}"),
        "application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml",
        job,
    )?;
    let bytes = package.get("xl/workbook.xml")?;
    let xml = Xml::parse(bytes, job)?;
    let parent = xml
        .child(xml.root, "sheets")
        .ok_or_else(|| error("Liste des feuilles absente."))?;
    let maximum = xml
        .children(parent, "sheet")
        .map(|i| {
            xml.nodes[i]
                .attr("sheetId")
                .unwrap_or("0")
                .parse::<u32>()
                .unwrap_or(0)
        })
        .max()
        .unwrap_or(0);
    let id = maximum
        .checked_add(1)
        .ok_or_else(|| error("Identifiant de feuille hors limites."))?;
    let tag = xml.nodes[parent].qualified("sheet");
    let child = format!(
        "<{tag} xmlns:r=\"{REL_NS}\" name=\"{}\" sheetId=\"{id}\" r:id=\"{rid}\"/>",
        escape(&name)
    );
    let output = append_children(bytes, &xml.nodes[parent], &child, &[])?;
    package.parts.insert("xl/workbook.xml".into(), output);
    package.parts.insert(path, report_sheet(rows, job)?);
    Ok(name)
}

fn force_recalculation(package: &mut Package, job: &JobControl) -> Result<()> {
    let bytes = package.get("xl/workbook.xml")?;
    let xml = Xml::parse(bytes, job)?;
    let output = if let Some(id) = xml.child(xml.root, "calcPr") {
        let node = &xml.nodes[id];
        patch(
            bytes,
            vec![(
                node.start,
                node.open_end,
                node.tag(
                    &[
                        ("fullCalcOnLoad", Some("1".into())),
                        ("forceFullCalc", Some("1".into())),
                    ],
                    node.empty,
                )
                .into_bytes(),
            )],
        )?
    } else {
        let root = &xml.nodes[xml.root];
        let name = root.qualified("calcPr");
        let position = root
            .children
            .iter()
            .find(|i| {
                matches!(
                    xml.nodes[**i].local(),
                    "oleSize"
                        | "customWorkbookViews"
                        | "pivotCaches"
                        | "smartTagPr"
                        | "smartTagTypes"
                        | "webPublishing"
                        | "fileRecoveryPr"
                        | "webPublishObjects"
                        | "extLst"
                )
            })
            .map_or(root.close_start, |id| xml.nodes[*id].start);
        patch(
            bytes,
            vec![(
                position,
                position,
                format!("<{name} fullCalcOnLoad=\"1\" forceFullCalc=\"1\"/>").into_bytes(),
            )],
        )?
    };
    package.parts.insert("xl/workbook.xml".into(), output);
    Ok(())
}

fn validate_changes(
    config: &ProjectConfig,
    result: &AnalysisResult,
    source: &SheetData,
    bytes: &[u8],
    job: &JobControl,
) -> Result<BTreeMap<(u32, u32), CellValue>> {
    if result.sheet != config.sheet
        || result.column_count != source.columns
        || result.row_count != source.rows.saturating_sub(config.header_row)
    {
        return Err(error(
            "Resultat obsolete ou dimensions d'analyse incoherentes.",
        ));
    }
    let allowed: BTreeSet<_> = config
        .mappings
        .iter()
        .filter(|m| m.confirmed && m.field_code.is_some())
        .map(|m| m.column)
        .chain(if config.segmentation.enabled {
            vec![
                config.segmentation.name_column,
                config.segmentation.bp_column,
                config.segmentation.phone_column,
                config.segmentation.location_column,
            ]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
        } else {
            vec![]
        })
        .collect();
    let xml = Xml::parse(bytes, job)?;
    let mut protected_ranges = Vec::new();
    for node in &xml.nodes {
        if node.local() == "f" || node.local() == "mergeCell" {
            if let Some(range) = node.attr("ref") {
                let (start, end) = range.split_once(':').unwrap_or((range, range));
                protected_ranges.push((
                    coordinate(start)?,
                    coordinate(end)?,
                    node.local() == "mergeCell",
                ));
            }
        }
    }
    let mut changes = BTreeMap::new();
    for change in &result.changes {
        job.check()?;
        let point = (change.row, change.column);
        if change.row <= config.header_row
            || change.row > source.rows
            || change.column == 0
            || change.column > source.columns
            || change.key != cell_key(&config.sheet, change.row, change.column)
            || !allowed.contains(&change.column)
            || !equivalent(&change.before, &source.cell(change.row, change.column))
        {
            return Err(error(
                "Correction hors perimetre autorise ou valeur source incoherente.",
            ));
        }
        if source.cell(change.row, change.column).kind == "formula"
            || protected_ranges.iter().any(|(a, b, merged)| {
                point.0 >= a.0
                    && point.0 <= b.0
                    && point.1 >= a.1
                    && point.1 <= b.1
                    && (!merged || point != *a)
            })
        {
            return Err(error(
                "Une formule ou une cellule couverte par une fusion ne peut pas etre remplacee.",
            ));
        }
        export_precision(&change.after)?;
        if changes.insert(point, change.after.clone()).is_some() {
            return Err(error("Plusieurs corrections pour la meme cellule."));
        }
    }
    for original in &result.originals {
        if original.key != cell_key(&config.sheet, original.row, original.column)
            || !equivalent(
                &original.before,
                &source.cell(original.row, original.column),
            )
        {
            return Err(error("Journal des valeurs sources incoherent."));
        }
    }
    for finding in &result.findings {
        let point = (finding.row, finding.column);
        let expected = changes
            .get(&point)
            .cloned()
            .unwrap_or_else(|| source.cell(point.0, point.1));
        if finding.key != cell_key(&config.sheet, point.0, point.1)
            || point.0 <= config.header_row
            || point.0 > source.rows
            || point.1 == 0
            || point.1 > source.columns
            || !equivalent(&finding.original, &source.cell(point.0, point.1))
            || !equivalent(&finding.current, &expected)
        {
            return Err(error(
                "Les anomalies ne correspondent pas au resultat final.",
            ));
        }
    }
    Ok(changes)
}

fn verify_sheet_structure(before: &[u8], after: &[u8], job: &JobControl) -> Result<()> {
    let before = normalize_inline(before, job)?;
    let left = Xml::parse(&before, job)?;
    let right = Xml::parse(after, job)?;
    let left_data = left
        .child(left.root, "sheetData")
        .ok_or_else(|| error("Donnees sources absentes."))?;
    let right_data = right
        .child(right.root, "sheetData")
        .ok_or_else(|| error("Donnees exportees absentes."))?;
    let envelope = |bytes: &[u8], node: &Node| {
        patch(
            bytes,
            vec![(node.start, node.end, node.tag(&[], true).into_bytes())],
        )
    };
    if envelope(&before, &left.nodes[left_data])? != envelope(after, &right.nodes[right_data])? {
        return Err(error("Structure de feuille modifiee : dimensions, fusions, filtres, volets ou autres proprietes."));
    }
    let right_rows: BTreeMap<_, _> = right
        .children(right_data, "row")
        .map(|id| (right.nodes[id].attr("r").unwrap_or(""), id))
        .collect();
    for row_id in left.children(left_data, "row") {
        job.check()?;
        let row = &left.nodes[row_id];
        let after_id = *right_rows
            .get(row.required("r")?)
            .ok_or_else(|| error("Ligne source perdue a l'export."))?;
        let after_row = &right.nodes[after_id];
        if row.attrs != after_row.attrs {
            return Err(error("Proprietes de ligne modifiees sans autorisation."));
        }
        let right_cells: BTreeMap<_, _> = right
            .children(after_id, "c")
            .map(|id| (right.nodes[id].attr("r").unwrap_or(""), id))
            .collect();
        for cell_id in left.children(row_id, "c") {
            let cell = &left.nodes[cell_id];
            let after_id = *right_cells
                .get(cell.required("r")?)
                .ok_or_else(|| error("Cellule source perdue a l'export."))?;
            let metadata = |node: &Node| {
                node.attrs
                    .iter()
                    .filter(|(key, _)| key != "t" && key != "s")
                    .cloned()
                    .collect::<BTreeMap<_, _>>()
            };
            if metadata(cell) != metadata(&right.nodes[after_id]) {
                return Err(error("Metadonnees de cellule modifiees."));
            }
            if left.child(cell_id, "f").is_some() {
                let formula = &before[cell.open_end..cell.close_start];
                let other = &right.nodes[after_id];
                if formula != &after[other.open_end..other.close_start] {
                    return Err(error(
                        "Formule ou cache de formule modifie sans autorisation.",
                    ));
                }
            }
        }
    }
    Ok(())
}

fn verify(
    package: &Package,
    actual: &Package,
    original: &Package,
    source: &WorkbookData,
    config: &ProjectConfig,
    changes: &BTreeMap<(u32, u32), CellValue>,
    reports: &BTreeMap<String, Vec<Vec<CellValue>>>,
    job: &JobControl,
) -> Result<()> {
    // Exact uncompressed-part comparison checks all structures, relationships and
    // metadata, including those that the typed cell model intentionally omits.
    if package.parts != actual.parts {
        return Err(error("Le fichier rouvert differe du paquet Excel attendu."));
    }
    for part in sheet_parts(original, job)? {
        if reports.contains_key(&part.name) && config.report_collision == "replace" {
            continue;
        }
        verify_sheet_structure(original.get(&part.path)?, actual.get(&part.path)?, job)?;
    }
    let output = data_from_package(actual, String::new(), job)?;
    for before in &source.sheets {
        job.check()?;
        if reports.contains_key(&before.name) && config.report_collision == "replace" {
            continue;
        }
        let after = output
            .sheets
            .iter()
            .find(|s| s.name == before.name)
            .ok_or_else(|| error("Feuille perdue a l'export."))?;
        if before.rows != after.rows || before.columns != after.columns {
            return Err(error("Dimensions modifiees sans autorisation."));
        }
        let coordinates: BTreeSet<_> = before
            .cells
            .keys()
            .chain(after.cells.keys())
            .copied()
            .collect();
        for (index, point) in coordinates.into_iter().enumerate() {
            if index % 4096 == 0 {
                job.check()?;
            }
            let expected = if before.name == config.sheet {
                changes
                    .get(&point)
                    .cloned()
                    .unwrap_or_else(|| before.cell(point.0, point.1))
            } else {
                before.cell(point.0, point.1)
            };
            if !equivalent(&expected, &after.cell(point.0, point.1)) {
                return Err(error(format!(
                    "Valeur non conservee : {}!{}.",
                    before.name,
                    address(point.0, point.1)
                )));
            }
        }
    }
    for (name, rows) in reports {
        let sheet = output
            .sheets
            .iter()
            .find(|s| &s.name == name)
            .ok_or_else(|| error("Rapport manquant."))?;
        if sheet.rows as usize != rows.len() {
            return Err(error("Nombre de lignes du rapport incoherent."));
        }
        for (r, values) in rows.iter().enumerate() {
            job.check()?;
            for (c, value) in values.iter().enumerate() {
                if !equivalent(value, &sheet.cell(r as u32 + 1, c as u32 + 1)) {
                    return Err(error("Valeur du rapport incoherente."));
                }
            }
        }
    }
    let worksheet_paths: BTreeSet<_> = sheet_parts(actual, job)?
        .into_iter()
        .map(|part| part.path)
        .collect();
    for (name, bytes) in &actual.parts {
        if name != "xl/styles.xml" && !worksheet_paths.contains(name) {
            continue;
        }
        let xml = Xml::parse(bytes, job)?;
        for node in &xml.nodes {
            if node.local() == "alignment" && node.attr("textRotation").is_some_and(|v| v != "0") {
                return Err(error("Rotation du texte restante."));
            }
            if node.local() == "numFmt"
                && node
                    .attr("formatCode")
                    .is_some_and(|v| remove_grouping(v) != v)
            {
                return Err(error("Groupement numerique restant."));
            }
            if node.local() == "xf" {
                if let Some(id) = node.attr("numFmtId").and_then(|v| v.parse::<u32>().ok()) {
                    if builtin_format(id).is_some_and(|code| remove_grouping(code) != code) {
                        return Err(error("Style integre avec groupement restant."));
                    }
                }
            }
        }
    }
    Ok(())
}

pub fn export_excel(
    config: &ProjectConfig,
    result: &AnalysisResult,
    destination: &Path,
    allow_issues: bool,
    overwrite: bool,
    job: &JobControl,
) -> Result<PathBuf> {
    job.check()?;
    extension(destination, "xlsx")?;
    protect_paths(
        destination,
        &[
            &config.source_path,
            &config.original_path,
            &config.dictionary_path,
        ],
    )?;
    let parent = destination
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    if !parent.is_dir() {
        return Err(error("Le dossier de destination n'existe pas."));
    }
    if destination.exists() && !overwrite {
        return Err(error(
            "Ce fichier existe deja. Confirmez explicitement son remplacement.",
        ));
    }
    let target_hash = if destination.exists() {
        Some(hash_file(destination, job)?)
    } else {
        None
    };
    engine::validate_config(config)?;
    if engine::config_hash(config)? != result.config_hash {
        return Err(error(
            "Resultat obsolete : relancez l'analyse avant d'exporter.",
        ));
    }
    if !result.findings.is_empty() && !allow_issues {
        return Err(error(
            "Des anomalies restent presentes. Validez explicitement leur export.",
        ));
    }
    if !matches!(config.report_collision.as_str(), "suffix" | "replace")
        || config
            .reports
            .iter()
            .any(|v| !matches!(v.as_str(), "control" | "original" | "verify"))
    {
        return Err(error("Selection ou politique de rapports invalide."));
    }
    job.report("export", 0, Some(5), "Preparation de la copie preservee");
    let source_path = Path::new(&config.source_path);
    extension(source_path, "xlsx")?;
    let bytes = read_bounded(source_path, MAX_FILE, job)?;
    if digest(&bytes) != result.source_hash {
        return Err(error("La source a change : resultat obsolete."));
    }
    let original = Package::from_bytes(&bytes, job)?;
    let source = data_from_package(&original, result.source_hash.clone(), job)?;
    let parts = sheet_parts(&original, job)?;
    let selected = parts
        .iter()
        .find(|s| s.name == config.sheet)
        .ok_or_else(|| error("Feuille selectionnee introuvable."))?;
    let selected_data = source
        .sheets
        .iter()
        .find(|s| s.name == config.sheet)
        .ok_or_else(|| error("Feuille selectionnee introuvable."))?;
    let changes = validate_changes(
        config,
        result,
        selected_data,
        original.get(&selected.path)?,
        job,
    )?;

    let book = umya_read(&bytes, job)?;
    let mut candidate = Vec::new();
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        umya_spreadsheet::writer::xlsx::write_writer(&book, &mut candidate)
    }))
    .map_err(|_| error("Ecriture Excel refusee ; source intacte."))?
    .map_err(|e| io_error("Ecriture Excel impossible", e))?;
    job.check()?;
    let mut package = Package::from_bytes(&candidate, job)?;
    // umya's lazy round-trip preserves worksheet XML, but its metadata/style writer
    // may normalize or omit unsupported fields. Restore original parts before making
    // narrowly scoped structured edits. Never accept a library round-trip as proof
    // of numeric or structural preservation.
    package
        .parts
        .retain(|name, _| original.parts.contains_key(name));
    for (name, content) in &original.parts {
        if package.parts.get(name) != Some(content) {
            package.parts.insert(name.clone(), content.clone());
        }
    }
    drop(candidate);
    drop(book);
    ensure_styles(&mut package, job)?;
    job.report("export", 1, Some(5), "Formats horizontaux sans groupement");
    let style_bytes = harmonize_styles(package.get("xl/styles.xml")?, job)?;
    package.parts.insert("xl/styles.xml".into(), style_bytes);
    let mut styles = StyleEditor::new(&package, job)?;
    for part in &parts {
        job.check()?;
        let data = source
            .sheets
            .iter()
            .find(|s| s.name == part.name)
            .ok_or_else(|| error("Feuille introuvable."))?;
        let no_changes = BTreeMap::new();
        let edits = if part.name == config.sheet {
            &changes
        } else {
            &no_changes
        };
        let output = write_sheet(
            package.get(&part.path)?,
            data,
            edits,
            config,
            &mut styles,
            job,
        )?;
        package.parts.insert(part.path.clone(), output);
    }
    package.parts.insert("xl/styles.xml".into(), styles.bytes);
    let mut reports = BTreeMap::new();
    if config.harmonized {
        for (code, base) in [
            ("control", "Contr\u{f4}le"),
            ("original", "Nom_original"),
            ("verify", "\u{c0} v\u{e9}rifier"),
        ] {
            if config.reports.iter().any(|v| v == code) {
                let rows = report_rows(code, config, result);
                let name = add_report(&mut package, base, &rows, config, job)?;
                reports.insert(name, rows);
            }
        }
    }
    force_recalculation(&mut package, job)?;
    let worksheet_paths: BTreeSet<_> = sheet_parts(&package, job)?
        .into_iter()
        .map(|part| part.path)
        .collect();
    for (name, bytes) in &mut package.parts {
        if name == "xl/styles.xml" || worksheet_paths.contains(name) {
            *bytes = normalize_inline(bytes, job)?;
        }
    }
    job.check()?;
    let mut temp = tempfile::Builder::new()
        .prefix(".fdx-")
        .suffix(".xlsx")
        .tempfile_in(parent)?;
    job.report("export", 2, Some(5), "Ecriture du classeur temporaire");
    package.write(temp.as_file_mut(), job)?;
    temp.flush()?;
    temp.as_file().sync_all()?;
    job.check()?;
    job.report(
        "export",
        3,
        Some(5),
        "Reouverture et verification des valeurs et structures",
    );
    let reopened = read_bounded(temp.path(), MAX_FILE, job)?;
    let actual = Package::from_bytes(&reopened, job)?;
    verify(
        &package, &actual, &original, &source, config, &changes, &reports, job,
    )?;
    let reopened_book = umya_read(&reopened, job)?;
    let expected_names = sheet_parts(&actual, job)?;
    if reopened_book
        .sheet_collection_no_check()
        .iter()
        .map(|s| s.name())
        .ne(expected_names.iter().map(|s| s.name.as_str()))
    {
        return Err(error(
            "Structure des feuilles non verifiee par le lecteur Excel.",
        ));
    }
    drop(reopened_book);
    job.report("export", 4, Some(5), "Verification finale de la source");
    if hash_file(source_path, job)? != result.source_hash {
        return Err(error(
            "La source a change pendant l'export ; fichier final non publie.",
        ));
    }
    protect_paths(
        destination,
        &[
            &config.source_path,
            &config.original_path,
            &config.dictionary_path,
        ],
    )?;
    job.check()?;
    match target_hash {
        Some(expected) => {
            if hash_file(destination, job)? != expected {
                return Err(error("La destination a change pendant l'export."));
            }
            job.check()?;
            temp.persist(destination)
                .map_err(|e| io_error("Publication impossible (fichier verrouille ?)", e.error))?;
        }
        None => {
            temp.persist_noclobber(destination).map_err(|e| {
                io_error(
                    "La destination existe deja ou ne peut pas etre creee",
                    e.error,
                )
            })?;
        }
    }
    job.report("export", 5, Some(5), "Export verifie");
    Ok(destination.to_owned())
}
