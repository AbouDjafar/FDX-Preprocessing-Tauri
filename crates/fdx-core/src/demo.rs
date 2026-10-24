use crate::models::{error, Result};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};
use zip::{write::SimpleFileOptions, ZipWriter};

pub fn create(folder: &Path, count: u32) -> Result<(PathBuf, PathBuf)> {
    fs::create_dir_all(folder)?;
    let source = folder.join("Exemple fictif.xlsx");
    let dictionary = folder.join("Dictionnaire exemple.md");
    let mut xml = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?><worksheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\"><sheetViews><sheetView workbookViewId=\"0\"><pane ySplit=\"1\" topLeftCell=\"A2\" activePane=\"bottomLeft\" state=\"frozen\"/></sheetView></sheetViews><cols><col min=\"1\" max=\"1\" width=\"28\" customWidth=\"1\"/></cols><sheetData>");
    let headers = [
        "Entreprise",
        "Telephone",
        "Montant",
        "Date de signature",
        "Identifiant",
    ];
    let templates = [
        [
            "  Atelier Horizon  ",
            "699123456 / 677123456",
            "1 234,50",
            "18/02/2025",
            "000123",
        ],
        [
            "Societe Exemple",
            "699123456",
            "27 650,125",
            "31/09/2025",
            "000124",
        ],
        [
            "Entreprise Demo",
            "+390498976845",
            "1250",
            "14/03/2025",
            "000125",
        ],
    ];
    for row in 1..=count.max(3) + 1 {
        xml.push_str(&format!("<row r=\"{row}\">"));
        let values = if row == 1 {
            &headers
        } else {
            &templates[((row - 2) % 3) as usize]
        };
        for (index, value) in values.iter().enumerate() {
            let col = (b'A' + index as u8) as char;
            let style = if row == 1 { " s=\"1\"" } else { "" };
            xml.push_str(&format!("<c r=\"{col}{row}\" t=\"inlineStr\"{style}><is><t xml:space=\"preserve\">{value}</t></is></c>"));
        }
        xml.push_str("</row>");
    }
    xml.push_str("</sheetData></worksheet>");
    let entries = [
        (
            "[Content_Types].xml",
            r#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/><Override PartName="/xl/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"#,
        ),
        (
            "xl/workbook.xml",
            r#"<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="Marches fictifs" sheetId="1" r:id="rId1"/></sheets></workbook>"#,
        ),
        (
            "xl/_rels/workbook.xml.rels",
            r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/></Relationships>"#,
        ),
        (
            "xl/styles.xml",
            r#"<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><fonts count="1"><font><sz val="11"/><name val="Calibri"/></font></fonts><fills count="2"><fill><patternFill patternType="none"/></fill><fill><patternFill patternType="gray125"/></fill></fills><borders count="1"><border/></borders><cellStyleXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/></cellStyleXfs><cellXfs count="2"><xf numFmtId="0" fontId="0" fillId="0" borderId="0" xfId="0"/><xf numFmtId="0" fontId="0" fillId="0" borderId="0" xfId="0" applyAlignment="1"><alignment textRotation="90"/></xf></cellXfs><cellStyles count="1"><cellStyle name="Normal" xfId="0" builtinId="0"/></cellStyles></styleSheet>"#,
        ),
        ("xl/worksheets/sheet1.xml", xml.as_str()),
    ];
    let mut archive = ZipWriter::new(fs::File::create(&source)?);
    for (name, content) in entries {
        archive
            .start_file(
                name,
                SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated),
            )
            .map_err(|e| error(e.to_string()))?;
        archive.write_all(content.as_bytes())?;
    }
    archive.finish().map_err(|e| error(e.to_string()))?;
    fs::write(&dictionary, "| Code | Libelle | Type | Taille | Obligatoire |\n|---|---|---|---|---|\n| NOM | Entreprise | Texte | 100 | Oui |\n| TEL | Telephone | Texte | 40 | Non |\n| MON | Montant | Decimal | | Oui |\n| DATE | Date de signature | Date | | Non |\n| ID | Identifiant | Texte | 10 | Non |\n")?;
    Ok((source, dictionary))
}
