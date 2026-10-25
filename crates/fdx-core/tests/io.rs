use fdx_core::{
    engine,
    exporter::{export_excel, remove_grouping},
    models::*,
    storage::*,
    workbooks::*,
};
use rusqlite::{params, Connection};
use serde_json::json;
use std::{
    collections::BTreeMap,
    fs,
    io::{Cursor, Read, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use tempfile::TempDir;
use zip::{write::SimpleFileOptions, CompressionMethod, ZipArchive, ZipWriter};

const NS: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
const REL: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const DEFAULT_SLICER_EXTENSION: &str = r#"<extLst><ext uri="{EB79DEF2-80B8-43e5-95BD-54CBDDF9020C}" xmlns:x14="http://schemas.microsoft.com/office/spreadsheetml/2009/9/main"><x14:slicerStyles defaultSlicerStyle="SlicerStyleLight1"/></ext></extLst>"#;

fn zip_bytes(parts: &BTreeMap<String, Vec<u8>>) -> Vec<u8> {
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in parts {
        zip.start_file(
            name,
            SimpleFileOptions::default().compression_method(CompressionMethod::Deflated),
        )
        .unwrap();
        zip.write_all(bytes).unwrap();
    }
    zip.finish().unwrap().into_inner()
}

fn parts(path: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut zip = ZipArchive::new(Cursor::new(fs::read(path).unwrap())).unwrap();
    let mut result = BTreeMap::new();
    for index in 0..zip.len() {
        let mut file = zip.by_index(index).unwrap();
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).unwrap();
        result.insert(file.name().to_string(), bytes);
    }
    result
}

fn edit(path: &Path, part: &str, transform: impl FnOnce(String) -> String) {
    let mut entries = parts(path);
    let text = String::from_utf8(entries.remove(part).unwrap()).unwrap();
    entries.insert(part.into(), transform(text).into_bytes());
    fs::write(path, zip_bytes(&entries)).unwrap();
}

fn xml(path: &Path, part: &str) -> String {
    String::from_utf8(parts(path).remove(part).unwrap()).unwrap()
}

fn synthetic() -> BTreeMap<String, Vec<u8>> {
    let mut result = BTreeMap::new();
    let content = r#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml"/><Override PartName="/xl/sharedStrings.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sharedStrings+xml"/><Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/><Override PartName="/xl/worksheets/sheet2.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/></Types>"#;
    result.insert("[Content_Types].xml".into(), content.as_bytes().to_vec());
    result.insert("_rels/.rels".into(), format!("<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"{REL}/officeDocument\" Target=\"xl/workbook.xml\"/></Relationships>").into_bytes());
    result.insert("xl/_rels/workbook.xml.rels".into(), format!("<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"{REL}/worksheet\" Target=\"worksheets/sheet1.xml\"/><Relationship Id=\"rId2\" Type=\"{REL}/worksheet\" Target=\"/xl/worksheets/sheet2.xml\"/><Relationship Id=\"rId3\" Type=\"{REL}/styles\" Target=\"styles.xml\"/><Relationship Id=\"rId4\" Type=\"{REL}/sharedStrings\" Target=\"sharedStrings.xml\"/></Relationships>").into_bytes());
    result.insert("xl/workbook.xml".into(), format!("<workbook xmlns=\"{NS}\" xmlns:r=\"{REL}\"><workbookPr date1904=\"0\"/><bookViews><workbookView activeTab=\"0\"/></bookViews><sheets><sheet name=\"Donnees\" sheetId=\"1\" r:id=\"rId1\"/><sheet name=\"Archive\" sheetId=\"7\" state=\"hidden\" r:id=\"rId2\"/></sheets><definedNames><definedName name=\"Zone\">Donnees!$A$1:$F$4</definedName><definedName name=\"_xlnm.Print_Area\" localSheetId=\"1\">Archive!$A$1:$B$2</definedName></definedNames><calcPr calcId=\"191029\"/></workbook>").into_bytes());
    result.insert("xl/styles.xml".into(), format!(r##"<styleSheet xmlns="{NS}"><numFmts count="2"><numFmt numFmtId="164" formatCode="0.0%"/><numFmt numFmtId="165" formatCode="0.00,,&quot;M&quot;"/></numFmts><fonts count="1"><font><sz val="11"/><name val="Calibri"/></font></fonts><fills count="2"><fill><patternFill patternType="none"/></fill><fill><patternFill patternType="gray125"/></fill></fills><borders count="1"><border><left/><right/><top/><bottom/><diagonal/></border></borders><cellStyleXfs count="1"><xf numFmtId="3" fontId="0" fillId="0" borderId="0"><alignment textRotation="90"/></xf></cellStyleXfs><cellXfs count="5"><xf numFmtId="0" fontId="0" fillId="0" borderId="0" xfId="0"><alignment textRotation="90"/></xf><xf numFmtId="4" fontId="0" fillId="0" borderId="0" xfId="0" applyNumberFormat="1"><alignment textRotation="255" horizontal="right"/></xf><xf numFmtId="164" fontId="0" fillId="0" borderId="0" xfId="0" applyNumberFormat="1"><alignment textRotation="15"/></xf><xf numFmtId="14" fontId="0" fillId="0" borderId="0" xfId="0" applyNumberFormat="1"/><xf numFmtId="165" fontId="0" fillId="0" borderId="0" xfId="0" applyNumberFormat="1"/></cellXfs><cellStyles count="1"><cellStyle name="Normal" xfId="0" builtinId="0"/></cellStyles><dxfs count="1"><dxf><numFmt numFmtId="166" formatCode="#,##0.000;[Red]-#,##0.000"/><alignment textRotation="30"/></dxf></dxfs></styleSheet>"##).into_bytes());
    result.insert("xl/sharedStrings.xml".into(), format!("<sst xmlns=\"{NS}\" count=\"1\" uniqueCount=\"1\"><si><r><rPr><b/></rPr><t>001</t></r><r><t>23</t></r><rPh sb=\"0\" eb=\"1\"><t>phonetic-not-data</t></rPh></si></sst>").into_bytes());
    result.insert("xl/worksheets/sheet1.xml".into(), format!(r#"<worksheet xmlns="{NS}"><sheetPr><pageSetUpPr fitToPage="1"/></sheetPr><dimension ref="A1:F4"/><sheetViews><sheetView workbookViewId="0"><pane ySplit="1" topLeftCell="A2" activePane="bottomLeft" state="frozen"/></sheetView></sheetViews><sheetFormatPr defaultRowHeight="15"/><cols><col min="2" max="2" width="23" customWidth="1" style="1"/></cols><sheetData><row r="1"><c r="A1" t="inlineStr"><is><t>Nom</t></is></c><c r="B1" t="inlineStr"><is><t>Montant</t></is></c><c r="C1" t="inlineStr"><is><t>Identifiant</t></is></c><c r="D1" t="inlineStr"><is><t>Date</t></is></c><c r="E1" t="inlineStr"><is><t>Formule</t></is></c><c r="F1" t="inlineStr"><is><t>Destination</t></is></c></row><row r="2"><c r="A2" t="inlineStr"><is><t xml:space="preserve">  Alpha   SA  </t></is></c><c r="B2" s="1" t="inlineStr"><is><t>1 234,50</t></is></c><c r="C2" t="s"><v>0</v></c><c r="D2" s="3"><v>46100</v></c><c r="E2"><f>SUM(B2:B3)</f><v>0</v></c></row><row r="3" s="1" customFormat="1" ht="25" customHeight="1"><c r="A3" t="inlineStr"><is><t>Beta</t></is></c><c r="B3" s="1"><v>1250.12345678901</v></c><c r="C3" t="inlineStr"><is><t>=not_a_formula</t></is></c><c r="D3" s="3"><v>9999999999</v></c><c r="E3" t="b"><v>1</v></c></row><row r="4"><c r="B4" s="2"><v>0.125</v></c><c r="D4"><v>1.234567890123456789E+20</v></c></row></sheetData><autoFilter ref="A1:F4"/><mergeCells count="1"><mergeCell ref="E4:F4"/></mergeCells><conditionalFormatting sqref="B2:B4"><cfRule type="cellIs" dxfId="0" priority="1" operator="greaterThan"><formula>0</formula></cfRule></conditionalFormatting><dataValidations count="1"><dataValidation type="whole" sqref="F2:F3" allowBlank="1"><formula1>0</formula1><formula2>100</formula2></dataValidation></dataValidations><pageMargins left="0.7" right="0.7" top="0.75" bottom="0.75" header="0.3" footer="0.3"/><pageSetup orientation="landscape" paperSize="9"/></worksheet>"#).into_bytes());
    result.insert("xl/worksheets/sheet2.xml".into(), format!("<worksheet xmlns=\"{NS}\"><dimension ref=\"A1:B2\"/><sheetViews><sheetView workbookViewId=\"0\"/></sheetViews><sheetData><row r=\"1\"><c r=\"A1\" t=\"inlineStr\"><is><t>Conserver</t></is></c></row><row r=\"2\"><c r=\"B2\" s=\"4\"><v>123456789.125</v></c></row></sheetData><mergeCells count=\"1\"><mergeCell ref=\"A1:B1\"/></mergeCells></worksheet>").into_bytes());
    result.insert("customXml/item1.xml".into(), b"<private-metadata><keep value=\"exact\"/><alignment textRotation=\"90\"/></private-metadata>".to_vec());
    result
}

struct Fixture {
    dir: TempDir,
    source: PathBuf,
    config: ProjectConfig,
}
impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source.xlsx");
        fs::write(&source, zip_bytes(&synthetic())).unwrap();
        let config = ProjectConfig {
            source_path: source.to_string_lossy().into_owned(),
            sheet: "Donnees".into(),
            fields: vec![
                FieldSpec {
                    code: "NOM".into(),
                    label: "Nom".into(),
                    required: true,
                    ..Default::default()
                },
                FieldSpec {
                    code: "MONTANT".into(),
                    label: "Montant".into(),
                    dtype: "decimal".into(),
                    ..Default::default()
                },
            ],
            mappings: vec![
                ColumnMapping {
                    column: 1,
                    field_code: Some("NOM".into()),
                    confirmed: true,
                    score: 100.0,
                },
                ColumnMapping {
                    column: 2,
                    field_code: Some("MONTANT".into()),
                    confirmed: true,
                    score: 100.0,
                },
            ],
            ..Default::default()
        };
        Self {
            dir,
            source,
            config,
        }
    }
    fn path(&self, name: &str) -> PathBuf {
        self.dir.path().join(name)
    }
    fn analyze(&self, decisions: Decisions) -> AnalysisResult {
        let data = read_source(&self.source, &JobControl::default()).unwrap();
        engine::analyze(&self.config, &data, &decisions, &JobControl::default()).unwrap()
    }
    fn export(&self, name: &str) -> PathBuf {
        let result = self.analyze(Decisions::new());
        export_excel(
            &self.config,
            &result,
            &self.path(name),
            true,
            false,
            &JobControl::default(),
        )
        .unwrap()
    }
}

fn no_temporary_files(dir: &Path) {
    assert!(!fs::read_dir(dir).unwrap().any(|e| e
        .unwrap()
        .file_name()
        .to_string_lossy()
        .starts_with(".fdx-")));
}

#[test]
fn reads_exact_numbers_rich_text_dates_and_formulas() {
    let fixture = Fixture::new();
    let source = read_source(&fixture.source, &JobControl::default()).unwrap();
    assert_eq!(source.source_hash, sha256(&fixture.source).unwrap());
    assert_eq!(source.sheets.len(), 2);
    let sheet = &source.sheets[0];
    assert_eq!((sheet.rows, sheet.columns), (4, 6));
    assert_eq!(
        sheet.cell(3, 2),
        CellValue::typed("decimal", "1250.12345678901")
    );
    assert_eq!(
        sheet.cell(4, 4),
        CellValue::typed("decimal", "1.234567890123456789E+20")
    );
    assert_eq!(sheet.cell(2, 3), CellValue::text("00123"));
    assert_eq!(sheet.cell(2, 4).kind, "date");
    assert_eq!(sheet.cell(3, 4), CellValue::typed("decimal", "9999999999"));
    assert_eq!(sheet.cell(2, 5), CellValue::typed("formula", "=SUM(B2:B3)"));
    assert_eq!(sheet.cell(3, 3), CellValue::text("=not_a_formula"));
    assert_eq!(sheet.cell(3, 5), CellValue::typed("boolean", "true"));
    assert!(sheet.cell(2, 6).is_empty());
}

#[test]
fn inspect_uses_all_sheets_and_one_based_header_preview() {
    let fixture = Fixture::new();
    let info = inspect(&fixture.source, 1, &JobControl::default()).unwrap();
    assert_eq!(info.suggested_sheet, "Donnees");
    assert_eq!(info.sheets[0].suggested_header, 1);
    assert_eq!(info.sheets[0].headers[0], CellValue::text("Nom"));
    assert_eq!(info.sheets[0].preview.len(), 3);
    assert!(inspect(&fixture.source, 0, &JobControl::default()).is_err());
    assert!(inspect(&fixture.source, 1_048_577, &JobControl::default()).is_err());
}

#[test]
fn date_epochs_and_fractional_serials_are_not_truncated() {
    let fixture = Fixture::new();
    edit(&fixture.source, "xl/workbook.xml", |x| {
        x.replace("date1904=\"0\"", "date1904=\"1\"")
    });
    edit(&fixture.source, "xl/worksheets/sheet1.xml", |x| {
        x.replace("<v>46100</v>", "<v>0</v>")
            .replace("<v>9999999999</v>", "<v>1.5</v>")
    });
    let data = read_source(&fixture.source, &JobControl::default()).unwrap();
    assert_eq!(
        data.sheets[0].cell(2, 4),
        CellValue::typed("date", "1904-01-01")
    );
    assert_eq!(
        data.sheets[0].cell(3, 4),
        CellValue::typed("decimal", "1.5")
    );
    edit(&fixture.source, "xl/workbook.xml", |x| {
        x.replace("date1904=\"1\"", "date1904=\"0\"")
    });
    edit(&fixture.source, "xl/worksheets/sheet1.xml", |x| {
        x.replace(
            "<c r=\"D2\" s=\"3\"><v>0</v></c>",
            "<c r=\"D2\" s=\"3\"><v>60</v></c>",
        )
    });
    let data = read_source(&fixture.source, &JobControl::default()).unwrap();
    assert_eq!(data.sheets[0].cell(2, 4), CellValue::typed("decimal", "60"));
    assert_eq!(data.sheets[0].cell(2, 3), CellValue::text("00123"));
    assert_eq!(
        data.sheets[0].cell(2, 5),
        CellValue::typed("formula", "=SUM(B2:B3)")
    );
}

#[test]
fn inherited_date_styles_are_read_without_float_conversion() {
    let fixture = Fixture::new();
    edit(&fixture.source, "xl/worksheets/sheet1.xml", |x| {
        x.replace("r=\"3\" s=\"1\"", "r=\"3\" s=\"3\"")
            .replace("r=\"D3\" s=\"3\"", "r=\"D3\"")
            .replace("<v>9999999999</v>", "<v>46101</v>")
    });
    assert_eq!(
        read_source(&fixture.source, &JobControl::default())
            .unwrap()
            .sheets[0]
            .cell(3, 4)
            .kind,
        "date"
    );
}

#[test]
fn export_preserves_all_values_structures_and_source() {
    let fixture = Fixture::new();
    let before = fs::read(&fixture.source).unwrap();
    let output = fixture.export("resultat.xlsx");
    assert_eq!(fs::read(&fixture.source).unwrap(), before);
    let data = read_source(&output, &JobControl::default()).unwrap();
    assert_eq!(
        data.sheets
            .iter()
            .map(|s| s.name.as_str())
            .collect::<Vec<_>>(),
        [
            "Donnees",
            "Archive",
            "Contr\u{f4}le",
            "Nom_original",
            "\u{c0} v\u{e9}rifier"
        ]
    );
    let sheet = &data.sheets[0];
    assert_eq!(sheet.cell(2, 1), CellValue::text("Alpha SA"));
    assert!(sheet
        .cell(2, 2)
        .equivalent(&CellValue::typed("decimal", "1234.5")));
    assert_eq!(sheet.cell(3, 2).value, "1250.12345678901");
    assert_eq!(sheet.cell(4, 4).value, "1.234567890123456789E+20");
    assert_eq!(sheet.cell(2, 5).kind, "formula");
    assert_eq!(sheet.cell(2, 3).value, "00123");
    let source_xml = xml(&fixture.source, "xl/worksheets/sheet1.xml");
    let exported = xml(&output, "xl/worksheets/sheet1.xml");
    for fragment in [
        "<dimension ref=\"A1:F4\"/>",
        "<autoFilter ref=\"A1:F4\"/>",
        "<mergeCell ref=\"E4:F4\"/>",
        "topLeftCell=\"A2\"",
        "ht=\"25\" customHeight=\"1\"",
        "<dataValidations count=\"1\">",
        "orientation=\"landscape\"",
    ] {
        assert!(source_xml.contains(fragment));
        assert!(exported.contains(fragment), "{fragment}");
    }
    assert_eq!(
        parts(&output)["customXml/item1.xml"],
        parts(&fixture.source)["customXml/item1.xml"]
    );
    let workbook = xml(&output, "xl/workbook.xml");
    assert!(workbook.contains("sheetId=\"7\" state=\"hidden\""));
    assert!(workbook.contains("localSheetId=\"1\""));
    assert!(workbook.contains("Donnees!$A$1:$F$4"));
    assert!(workbook.contains("forceFullCalc=\"1\""));
    no_temporary_files(fixture.dir.path());
}

#[test]
fn formats_normalize_cells_rows_columns_named_and_conditional_styles() {
    let fixture = Fixture::new();
    let output = fixture.export("formats.xlsx");
    let styles = xml(&output, "xl/styles.xml");
    for rotation in ["90", "255", "15", "30"] {
        assert!(!styles.contains(&format!("textRotation=\"{rotation}\"")));
    }
    assert!(!styles.contains("#,##"));
    assert!(styles.contains("horizontal=\"right\""));
    assert!(styles.contains("0.0%"));
    assert!(styles.contains(",,&quot;M&quot;"));
    assert!(styles.contains("[Red]"));
    assert!(styles.contains("0.00#########"));
    let data = read_source(&output, &JobControl::default()).unwrap();
    assert_eq!(data.sheets[0].cell(4, 2).value, "0.125");
    assert_eq!(data.sheets[1].cell(2, 2).value, "123456789.125");
}

#[test]
fn grouping_does_not_change_percent_scaling_literals_or_fractions() {
    for (input, expected) in [
        ("#,##0.00;[Red]-#,##0.00", "###0.00;[Red]-###0.00"),
        ("#,##0,,\" million, units\"", "###0,,\" million, units\""),
        ("0.00%", "0.00%"),
        ("0\"%\"", "0\"%\""),
        ("# ?/?", "# ?/?"),
        ("# ??/??", "# ??/??"),
        ("# ##0.000", "###0.000"),
        ("#\u{a0}##0", "###0"),
        ("0\\,000", "0\\,000"),
        ("[>=1000]#,##0.000E+00", "[>=1000]###0.000E+00"),
    ] {
        assert_eq!(remove_grouping(input), expected);
    }
}

#[test]
fn export_retains_negative_sections_scaling_and_quoted_units() {
    let fixture = Fixture::new();
    edit(&fixture.source, "xl/styles.xml", |x| {
        x.replace("0.00,,&quot;M&quot;", "#,##0.00,,&quot; M, units&quot;;[Red](#,##0.00,,&quot; M, units&quot;);&quot;zero; literal&quot;;@")
    });
    edit(&fixture.source, "xl/worksheets/sheet2.xml", |x| {
        x.replace("123456789.125", "-123456789.125")
    });
    let output = fixture.export("negative-scaled.xlsx");
    let styles = xml(&output, "xl/styles.xml");
    assert!(styles.contains("###0.00#######,,&quot; M, units&quot;;[Red](###0.00#######,,&quot; M, units&quot;);&quot;zero; literal&quot;;@"));
    assert_eq!(
        read_source(&output, &JobControl::default()).unwrap().sheets[1]
            .cell(2, 2)
            .value,
        "-123456789.125"
    );
}

#[test]
fn unsupported_custom_precision_refuses_export_instead_of_dropping_format_semantics() {
    let fixture = Fixture::new();
    edit(&fixture.source, "xl/worksheets/sheet2.xml", |x| {
        x.replace("123456789.125", "1e-40")
    });
    let result = fixture.analyze(Decisions::new());
    let output = fixture.path("tiny-scaled.xlsx");
    let err = export_excel(
        &fixture.config,
        &result,
        &output,
        true,
        false,
        &JobControl::default(),
    )
    .unwrap_err();
    assert!(err.to_string().contains("30 decimales"));
    assert!(!output.exists());
    no_temporary_files(fixture.dir.path());
}

#[test]
fn reports_selection_and_plain_mode_preserve_existing_report_names() {
    let mut fixture = Fixture::new();
    edit(&fixture.source, "xl/workbook.xml", |x| {
        x.replace("name=\"Archive\"", "name=\"\u{c0} v\u{e9}rifier\"")
    });
    fixture.config.reports = vec!["verify".into()];
    let output = fixture.export("selected.xlsx");
    let data = read_source(&output, &JobControl::default()).unwrap();
    assert_eq!(
        data.sheets
            .iter()
            .map(|s| s.name.as_str())
            .collect::<Vec<_>>(),
        [
            "Donnees",
            "\u{c0} v\u{e9}rifier",
            "\u{c0} v\u{e9}rifier (2)"
        ]
    );
    assert_eq!(data.sheets[1].cell(1, 1).value, "Conserver");
    assert_eq!(
        data.sheets[2].rows as usize - 1,
        fixture.analyze(Decisions::new()).findings.len()
    );
    fixture.config.harmonized = false;
    assert_eq!(
        read_source(&fixture.export("plain.xlsx"), &JobControl::default())
            .unwrap()
            .sheets
            .len(),
        2
    );
    fixture.config.harmonized = true;
    fixture.config.reports.clear();
    assert_eq!(
        read_source(&fixture.export("none.xlsx"), &JobControl::default())
            .unwrap()
            .sheets
            .len(),
        2
    );
}

#[test]
fn explicit_report_replacement_preserves_sheet_indexes_and_other_sheets() {
    let mut fixture = Fixture::new();
    edit(&fixture.source, "xl/workbook.xml", |x| {
        x.replace("name=\"Archive\"", "name=\"Nom_original\"")
    });
    fixture.config.report_collision = "replace".into();
    fixture.config.reports = vec!["original".into()];
    let output = fixture.export("replace.xlsx");
    let data = read_source(&output, &JobControl::default()).unwrap();
    assert_eq!(data.sheets.len(), 2);
    assert_eq!(data.sheets[1].cell(1, 1).value, "Feuille");
    assert!(xml(&output, "xl/workbook.xml").contains("localSheetId=\"1\""));
}

#[test]
fn report_cannot_replace_selected_data_sheet() {
    let mut fixture = Fixture::new();
    edit(&fixture.source, "xl/workbook.xml", |x| {
        x.replace("name=\"Donnees\"", "name=\"Nom_original\"")
    });
    fixture.config.sheet = "Nom_original".into();
    fixture.config.report_collision = "replace".into();
    let result = fixture.analyze(Decisions::new());
    assert!(export_excel(
        &fixture.config,
        &result,
        &fixture.path("bad.xlsx"),
        true,
        false,
        &JobControl::default()
    )
    .is_err());
    assert!(!fixture.path("bad.xlsx").exists());
}

#[test]
fn text_that_looks_like_formula_is_literal_in_data_and_reports() {
    let fixture = Fixture::new();
    let dangerous = "=HYPERLINK(\"https://invalid.example\",\"x\") _x0041_\r\n\u{1}";
    let decisions = BTreeMap::from([(
        cell_key("Donnees", 2, 1),
        Decision {
            action: "set".into(),
            value: CellValue::text(dangerous),
            ..Default::default()
        },
    )]);
    let result = fixture.analyze(decisions);
    let output = fixture.path("literal.xlsx");
    export_excel(
        &fixture.config,
        &result,
        &output,
        true,
        false,
        &JobControl::default(),
    )
    .unwrap();
    let data = read_source(&output, &JobControl::default()).unwrap();
    assert_eq!(data.sheets[0].cell(2, 1), CellValue::text(dangerous));
    for sheet in data.sheets.iter().skip(2) {
        assert!(sheet.cells.values().all(|v| v.kind != "formula"));
    }
}

#[test]
fn typed_date_boolean_and_empty_values_are_verified() {
    for value in [
        CellValue::typed("date", "2025-09-30"),
        CellValue::typed("boolean", "false"),
        CellValue::empty(),
    ] {
        let mut fixture = Fixture::new();
        fixture.config.fields[0].dtype = match value.kind.as_str() {
            "date" => "date",
            "boolean" => "boolean",
            _ => "text",
        }
        .into();
        let decisions = BTreeMap::from([(
            cell_key("Donnees", 2, 1),
            Decision {
                action: "set".into(),
                value: value.clone(),
                ..Default::default()
            },
        )]);
        let result = fixture.analyze(decisions);
        let output = fixture.path("typed.xlsx");
        export_excel(
            &fixture.config,
            &result,
            &output,
            true,
            false,
            &JobControl::default(),
        )
        .unwrap();
        assert!(
            read_source(&output, &JobControl::default()).unwrap().sheets[0]
                .cell(2, 1)
                .equivalent(&value)
        );
    }
}

#[test]
fn missing_cells_and_rows_can_be_written_without_resizing() {
    let mut fixture = Fixture::new();
    edit(&fixture.source, "xl/worksheets/sheet1.xml", |x| {
        x.replace("<row r=\"4\"><c r=\"B4\" s=\"2\"><v>0.125</v></c><c r=\"D4\"><v>1.234567890123456789E+20</v></c></row>", "")
    });
    fixture.config.segmentation = SegmentationRule {
        enabled: true,
        source_column: 1,
        name_column: Some(6),
        ..Default::default()
    };
    let decisions = BTreeMap::from([
        (
            cell_key("Donnees", 4, 1),
            Decision {
                action: "set".into(),
                value: CellValue::text("Added"),
                ..Default::default()
            },
        ),
        (
            cell_key("Donnees", 2, 6),
            Decision {
                action: "set".into(),
                value: CellValue::text("Destination"),
                ..Default::default()
            },
        ),
    ]);
    let result = fixture.analyze(decisions);
    let output = fixture.path("missing.xlsx");
    export_excel(
        &fixture.config,
        &result,
        &output,
        true,
        false,
        &JobControl::default(),
    )
    .unwrap();
    let data = read_source(&output, &JobControl::default()).unwrap();
    assert_eq!((data.sheets[0].rows, data.sheets[0].columns), (4, 6));
    assert_eq!(data.sheets[0].cell(4, 1).value, "Added");
    assert_eq!(data.sheets[0].cell(2, 6).value, "Destination");
}

#[test]
fn stale_config_source_and_protected_paths_are_refused() {
    let mut fixture = Fixture::new();
    let result = fixture.analyze(Decisions::new());
    assert!(export_excel(
        &fixture.config,
        &result,
        &fixture.source,
        true,
        true,
        &JobControl::default()
    )
    .is_err());
    fixture.config.original_path = fixture.path("original.xlsx").to_string_lossy().into_owned();
    fs::copy(&fixture.source, &fixture.config.original_path).unwrap();
    assert!(export_excel(
        &fixture.config,
        &result,
        Path::new(&fixture.config.original_path),
        true,
        true,
        &JobControl::default()
    )
    .is_err());
    fixture.config.original_path.clear();
    fixture.config.rules.insert(
        "NOM".into(),
        FieldRule {
            normalize_spaces: false,
            ..Default::default()
        },
    );
    assert!(export_excel(
        &fixture.config,
        &result,
        &fixture.path("stale.xlsx"),
        true,
        false,
        &JobControl::default()
    )
    .is_err());
    fixture.config.rules.clear();
    edit(&fixture.source, "xl/worksheets/sheet1.xml", |x| {
        x.replace("Beta", "Changed externally")
    });
    assert!(export_excel(
        &fixture.config,
        &result,
        &fixture.path("stale.xlsx"),
        true,
        false,
        &JobControl::default()
    )
    .is_err());
    assert!(!fixture.path("stale.xlsx").exists());
}

#[test]
fn hard_links_to_source_are_refused() {
    let fixture = Fixture::new();
    let alias = fixture.path("alias.xlsx");
    fs::hard_link(&fixture.source, &alias).unwrap();
    assert!(export_excel(
        &fixture.config,
        &fixture.analyze(Decisions::new()),
        &alias,
        true,
        true,
        &JobControl::default()
    )
    .is_err());
}

#[test]
fn anomaly_and_overwrite_consent_are_required() {
    let fixture = Fixture::new();
    let result = fixture.analyze(Decisions::new());
    assert!(!result.findings.is_empty());
    let output = fixture.path("existing.xlsx");
    assert!(export_excel(
        &fixture.config,
        &result,
        &output,
        false,
        false,
        &JobControl::default()
    )
    .is_err());
    fs::write(&output, b"existing contents").unwrap();
    assert!(export_excel(
        &fixture.config,
        &result,
        &output,
        true,
        false,
        &JobControl::default()
    )
    .is_err());
    assert_eq!(fs::read(&output).unwrap(), b"existing contents");
    export_excel(
        &fixture.config,
        &result,
        &output,
        true,
        true,
        &JobControl::default(),
    )
    .unwrap();
    read_source(&output, &JobControl::default()).unwrap();
}

#[test]
fn cancellation_before_read_and_during_verification_is_atomic() {
    let fixture = Fixture::new();
    let cancelled = Arc::new(AtomicBool::new(true));
    let job = JobControl {
        cancelled: cancelled.clone(),
        progress: None,
    };
    assert!(matches!(
        read_source(&fixture.source, &job),
        Err(FdxError::Cancelled)
    ));
    let result = fixture.analyze(Decisions::new());
    cancelled.store(false, Ordering::Relaxed);
    let flag = cancelled.clone();
    let job = JobControl {
        cancelled,
        progress: Some(Arc::new(move |p| {
            if p.phase == "export" && p.current == 3 {
                flag.store(true, Ordering::Relaxed);
            }
        })),
    };
    let output = fixture.path("cancelled.xlsx");
    fs::write(&output, b"keep me").unwrap();
    assert!(matches!(
        export_excel(&fixture.config, &result, &output, true, true, &job),
        Err(FdxError::Cancelled)
    ));
    assert_eq!(fs::read(&output).unwrap(), b"keep me");
    no_temporary_files(fixture.dir.path());
}

#[cfg(windows)]
#[test]
fn locked_windows_destination_refuses_overwrite_without_changing_files() {
    use std::os::windows::fs::OpenOptionsExt;

    let fixture = Fixture::new();
    let result = fixture.analyze(Decisions::new());
    let source_before = fs::read(&fixture.source).unwrap();
    let output = fixture.path("locked.xlsx");
    let target_before = b"Existing destination must remain intact";
    fs::write(&output, target_before).unwrap();

    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .share_mode(0)
        .open(&output)
        .unwrap();
    let exported = export_excel(
        &fixture.config,
        &result,
        &output,
        true,
        true,
        &JobControl::default(),
    );
    drop(lock);

    assert!(exported.is_err());
    assert_eq!(fs::read(&output).unwrap(), target_before);
    assert_eq!(fs::read(&fixture.source).unwrap(), source_before);
    no_temporary_files(fixture.dir.path());
}

#[test]
fn source_changed_during_export_is_not_published() {
    let fixture = Fixture::new();
    let result = fixture.analyze(Decisions::new());
    let source = fixture.source.clone();
    let job = JobControl {
        progress: Some(Arc::new(move |p| {
            if p.phase == "export" && p.current == 4 {
                fs::OpenOptions::new()
                    .append(true)
                    .open(&source)
                    .unwrap()
                    .write_all(b"changed")
                    .unwrap();
            }
        })),
        ..Default::default()
    };
    let output = fixture.path("race.xlsx");
    assert!(export_excel(&fixture.config, &result, &output, true, false, &job).is_err());
    assert!(!output.exists());
    no_temporary_files(fixture.dir.path());
}

#[test]
fn concurrent_destination_creation_or_change_is_not_overwritten() {
    for existing in [false, true] {
        let fixture = Fixture::new();
        let result = fixture.analyze(Decisions::new());
        let output = fixture.path("race.xlsx");
        if existing {
            fs::write(&output, b"old").unwrap();
        }
        let path = output.clone();
        let job = JobControl {
            progress: Some(Arc::new(move |p| {
                if p.phase == "export" && p.current == 4 {
                    fs::write(&path, b"concurrent").unwrap();
                }
            })),
            ..Default::default()
        };
        assert!(export_excel(&fixture.config, &result, &output, true, existing, &job).is_err());
        assert_eq!(fs::read(&output).unwrap(), b"concurrent");
        no_temporary_files(fixture.dir.path());
    }
}

#[test]
fn changed_numbers_beyond_excel_precision_are_rejected_not_rounded() {
    for raw in ["1234567890123456", "1.234567890123456", "1e-308", "1e308"] {
        let fixture = Fixture::new();
        let mut result = fixture.analyze(Decisions::new());
        result
            .changes
            .iter_mut()
            .find(|c| c.column == 2)
            .unwrap()
            .after = CellValue::typed("decimal", raw);
        let output = fixture.path("precision.xlsx");
        let err = export_excel(
            &fixture.config,
            &result,
            &output,
            true,
            false,
            &JobControl::default(),
        )
        .unwrap_err();
        assert!(err.to_string().contains("Precision"), "{err}");
        assert!(!output.exists());
    }
}

#[test]
fn forged_formula_unmapped_and_merge_changes_are_refused() {
    for (row, column, value) in [
        (2, 5, CellValue::text("overwrite")),
        (2, 3, CellValue::text("overwrite")),
        (4, 6, CellValue::text("overwrite")),
        (2, 1, CellValue::typed("formula", "=1+1")),
    ] {
        let fixture = Fixture::new();
        let mut result = fixture.analyze(Decisions::new());
        let source = read_source(&fixture.source, &JobControl::default()).unwrap();
        result
            .changes
            .retain(|c| (c.row, c.column) != (row, column));
        result.changes.push(CellChange {
            key: cell_key("Donnees", row, column),
            row,
            column,
            field_code: "NOM".into(),
            before: source.sheets[0].cell(row, column),
            after: value,
            reason: "invalid".into(),
        });
        assert!(export_excel(
            &fixture.config,
            &result,
            &fixture.path("forged.xlsx"),
            true,
            false,
            &JobControl::default()
        )
        .is_err());
    }
}

#[test]
fn empty_default_slicer_metadata_is_preserved_without_losing_numeric_precision() {
    let fixture = Fixture::new();
    edit(&fixture.source, "xl/styles.xml", |x| {
        x.replace(
            "</styleSheet>",
            &format!("{DEFAULT_SLICER_EXTENSION}</styleSheet>"),
        )
    });
    let before = fs::read(&fixture.source).unwrap();
    let output = fixture.export("default-style.xlsx");
    assert!(xml(&output, "xl/styles.xml").contains(DEFAULT_SLICER_EXTENSION));
    assert_eq!(fs::read(&fixture.source).unwrap(), before);
    let data = read_source(&output, &JobControl::default()).unwrap();
    assert_eq!(
        data.sheets[0].cell(4, 4),
        CellValue::typed("decimal", "1.234567890123456789E+20")
    );
    no_temporary_files(fixture.dir.path());
}

#[test]
fn umya_generated_workbook_can_be_read_and_exported() {
    let fixture = Fixture::new();
    let book = umya_spreadsheet::reader::xlsx::read_reader(
        Cursor::new(fs::read(&fixture.source).unwrap()),
        false,
    )
    .unwrap();
    let mut bytes = Vec::new();
    umya_spreadsheet::writer::xlsx::write_writer(&book, &mut bytes).unwrap();
    fs::write(&fixture.source, &bytes).unwrap();
    assert!(xml(&fixture.source, "xl/styles.xml").contains(DEFAULT_SLICER_EXTENSION));
    let source = read_source(&fixture.source, &JobControl::default()).unwrap();
    assert_eq!(source.sheets.len(), 2);
    let output = fixture.export("umya-output.xlsx");
    let exported = read_source(&output, &JobControl::default()).unwrap();
    assert_eq!(exported.sheets[0].cell(4, 4), source.sheets[0].cell(4, 4));
    assert_eq!(fs::read(&fixture.source).unwrap(), bytes);
    assert!(xml(&output, "xl/styles.xml").contains(DEFAULT_SLICER_EXTENSION));
    no_temporary_files(fixture.dir.path());
}

#[test]
fn slicer_extension_allowlist_rejects_payloads_namespace_spoofing_and_other_locations() {
    let extension = DEFAULT_SLICER_EXTENSION;
    let variants = [
        ("unknown URI", extension.replace("EB79DEF2", "FFFFFFFF")),
        (
            "wrong style namespace",
            extension.replace(
                "http://schemas.microsoft.com/office/spreadsheetml/2009/9/main",
                "urn:unsupported",
            ),
        ),
        (
            "wrong list namespace",
            extension.replace("<extLst>", "<extLst xmlns=\"urn:unsupported\">"),
        ),
        (
            "wrong ext namespace",
            extension.replace("<ext uri=", "<ext xmlns=\"urn:unsupported\" uri="),
        ),
        ("custom default", extension.replace("SlicerStyleLight1", "CustomStyle")),
        ("list attributes", extension.replace("<extLst>", "<extLst count=\"1\">")),
        ("ext attributes", extension.replace("<ext uri=", "<ext extra=\"1\" uri=")),
        (
            "style attributes",
            extension.replace("<x14:slicerStyles ", "<x14:slicerStyles count=\"1\" "),
        ),
        (
            "style definitions",
            extension.replace("/>", "><x14:slicerStyle name=\"CustomStyle\"/></x14:slicerStyles>"),
        ),
        (
            "style text",
            extension.replace("/>", ">payload</x14:slicerStyles>"),
        ),
        (
            "another extension",
            extension.replace("</extLst>", "<ext uri=\"unknown\"/></extLst>"),
        ),
        ("duplicate lists", format!("{extension}{extension}")),
        ("nested list", format!("<wrapper>{extension}</wrapper>")),
    ];
    for (label, invalid) in variants {
        let fixture = Fixture::new();
        edit(&fixture.source, "xl/styles.xml", |x| {
            x.replace("</styleSheet>", &format!("{invalid}</styleSheet>"))
        });
        let err = read_source(&fixture.source, &JobControl::default()).unwrap_err();
        assert!(err.to_string().contains("Extensions ou objets"), "{label}: {err}");
    }
    for (part, closing) in [
        ("xl/worksheets/sheet1.xml", "</worksheet>"),
        ("xl/workbook.xml", "</workbook>"),
        ("customXml/item1.xml", "</private-metadata>"),
    ] {
        let fixture = Fixture::new();
        edit(&fixture.source, part, |x| {
            x.replace(closing, &format!("{extension}{closing}"))
        });
        let err = read_source(&fixture.source, &JobControl::default()).unwrap_err();
        assert!(err.to_string().contains("Extensions ou objets"), "{part}: {err}");
    }
}

#[test]
fn unsafe_xml_is_refused_in_every_xml_part() {
    for part in [
        "xl/workbook.xml",
        "xl/styles.xml",
        "xl/sharedStrings.xml",
        "customXml/item1.xml",
    ] {
        let fixture = Fixture::new();
        edit(&fixture.source, part, |x| {
            format!("<!DOCTYPE forbidden [<!ENTITY secret SYSTEM 'file:///private'>]>{x}")
        });
        assert!(read_source(&fixture.source, &JobControl::default())
            .unwrap_err()
            .to_string()
            .contains("DTD"));
    }
    let fixture = Fixture::new();
    edit(&fixture.source, "xl/sharedStrings.xml", |x| {
        x.replace("001", "&unknown;")
    });
    assert!(read_source(&fixture.source, &JobControl::default()).is_err());
}

#[test]
fn unsupported_objects_prefixed_extensions_and_bad_zip_paths_are_rejected() {
    for path in [
        "xl/drawings/drawing1.xml",
        "xl/pivotTables/pivot1.xml",
        "xl/slicers/slicer1.xml",
        "xl/vbaProject.bin",
        "../escape.xml",
        "/absolute.xml",
        "xl/../escape.xml",
    ] {
        let fixture = Fixture::new();
        let mut entries = parts(&fixture.source);
        entries.insert(path.into(), b"<x/>".to_vec());
        fs::write(&fixture.source, zip_bytes(&entries)).unwrap();
        assert!(
            read_source(&fixture.source, &JobControl::default()).is_err(),
            "{path}"
        );
    }
    let fixture = Fixture::new();
    edit(&fixture.source, "xl/worksheets/sheet1.xml", |x| {
        x.replace("</worksheet>", "<s:extLst xmlns:s=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\"/></worksheet>")
    });
    assert!(read_source(&fixture.source, &JobControl::default()).is_err());
}

#[test]
fn zip_expansion_limits_and_case_duplicate_parts_are_rejected() {
    let fixture = Fixture::new();
    let mut entries = parts(&fixture.source);
    entries.insert("xl/WORKBOOK.xml".into(), entries["xl/workbook.xml"].clone());
    fs::write(&fixture.source, zip_bytes(&entries)).unwrap();
    assert!(read_source(&fixture.source, &JobControl::default())
        .unwrap_err()
        .to_string()
        .contains("duplique"));
    entries = synthetic();
    entries.insert("padding.bin".into(), vec![0; 4_000_000]);
    fs::write(&fixture.source, zip_bytes(&entries)).unwrap();
    assert!(read_source(&fixture.source, &JobControl::default())
        .unwrap_err()
        .to_string()
        .contains("Limite ZIP"));
}

#[test]
fn active_external_links_and_missing_relationship_targets_are_rejected() {
    let fixture = Fixture::new();
    edit(&fixture.source, "xl/_rels/workbook.xml.rels", |x| {
        x.replace("</Relationships>", &format!("<Relationship Id=\"bad\" Type=\"{REL}/externalLink\" Target=\"https://invalid.example\" TargetMode=\"External\"/></Relationships>"))
    });
    assert!(read_source(&fixture.source, &JobControl::default()).is_err());
    let fixture = Fixture::new();
    edit(&fixture.source, "xl/_rels/workbook.xml.rels", |x| {
        x.replace("worksheets/sheet1.xml", "worksheets/missing.xml")
    });
    assert!(read_source(&fixture.source, &JobControl::default()).is_err());
}

#[test]
fn cyclic_part_relationships_are_rejected_before_umya_recursion() {
    let fixture = Fixture::new();
    let mut entries = parts(&fixture.source);
    entries.insert("xl/worksheets/_rels/sheet1.xml.rels".into(), format!("<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"cycle\" Type=\"{REL}/worksheet\" Target=\"sheet1.xml\"/></Relationships>").into_bytes());
    fs::write(&fixture.source, zip_bytes(&entries)).unwrap();
    let err = read_source(&fixture.source, &JobControl::default()).unwrap_err();
    assert!(err.to_string().contains("cycliques"));
}

fn legacy_database(fixture: &Fixture, path: &Path, decisions: &[u8]) {
    let db = Connection::open(path).unwrap();
    db.execute(
        "CREATE TABLE project (key TEXT PRIMARY KEY, value BLOB NOT NULL)",
        [],
    )
    .unwrap();
    let mut config = serde_json::to_value(&fixture.config).unwrap();
    config["version"] = json!(1);
    config.as_object_mut().unwrap().remove("reports");
    for (key, value) in [
        ("config", serde_json::to_vec(&config).unwrap()),
        ("source", fs::read(&fixture.source).unwrap()),
        ("source_hash", sha256(&fixture.source).unwrap().into_bytes()),
        (
            "dictionary",
            b"Code\tLibelle\tType\nNOM\tNom\ttext".to_vec(),
        ),
        ("decisions", decisions.to_vec()),
        (
            "history",
            br#"[{"test":{"action":"keep"},"revision":1}]"#.to_vec(),
        ),
        (
            "ui_state",
            br#"{"mode":"advanced","view":"changes"}"#.to_vec(),
        ),
        ("result", b"ignored obsolete cached result".to_vec()),
    ] {
        db.execute("INSERT INTO project VALUES (?1, ?2)", params![key, value])
            .unwrap();
    }
    // Python v1 stored version/config/hash as SQLite TEXT, other values as BLOB.
    db.execute("INSERT INTO project VALUES ('version', '1')", [])
        .unwrap();
    db.execute(
        "UPDATE project SET value = CAST(value AS TEXT) WHERE key IN ('config', 'source_hash')",
        [],
    )
    .unwrap();
}

#[test]
fn legacy_project_import_retains_typed_decisions_source_dictionary_and_history() {
    let fixture = Fixture::new();
    let path = fixture.path("legacy.fdxproj");
    legacy_database(&fixture, &path, br#"{"Donnees!2:1":{"action":"set","value":"=1+1"},"Donnees!2:2":{"action":"set","value":"1234.50000000000000001","value_kind":"decimal"},"Donnees!3:2":{"action":"set","value":1234567890123456789012345},"Donnees!2:4":{"action":"set","value":"2025-09-30T00:00:00","value_kind":"date"}}"#);
    let before = sha256(&path).unwrap();
    let loaded = open_project(&path, &fixture.path("cache")).unwrap();
    assert_eq!(loaded.config.version, 2);
    assert_eq!(loaded.config.original_path, fixture.config.source_path);
    assert_ne!(loaded.config.source_path, fixture.config.source_path);
    assert_eq!(
        sha256(Path::new(&loaded.config.source_path)).unwrap(),
        sha256(&fixture.source).unwrap()
    );
    assert!(fs::read_to_string(&loaded.config.dictionary_path)
        .unwrap()
        .contains("NOM"));
    assert_eq!(
        loaded.decisions["Donnees!2:1"].value,
        CellValue::text("=1+1")
    );
    assert_eq!(
        loaded.decisions["Donnees!2:2"].value,
        CellValue::typed("decimal", "1234.50000000000000001")
    );
    assert_eq!(
        loaded.decisions["Donnees!3:2"].value,
        CellValue::typed("integer", "1234567890123456789012345")
    );
    assert_eq!(
        loaded.decisions["Donnees!2:4"].value,
        CellValue::typed("date", "2025-09-30")
    );
    assert_eq!(loaded.history[0]["test"]["action"], "keep");
    assert_eq!(loaded.ui_state["mode"], "advanced");
    assert_eq!(sha256(&path).unwrap(), before);
    assert!(save_project(&path, &loaded).is_err());
    assert_eq!(sha256(&path).unwrap(), before);
    let migrated = fixture.path("migrated.fdxproj");
    save_project(&migrated, &loaded).unwrap();
    let reopened = open_project(&migrated, &fixture.path("cache2")).unwrap();
    assert_eq!(reopened.decisions, loaded.decisions);
    assert_eq!(reopened.history, loaded.history);
}

#[test]
fn legacy_decimal_json_numbers_are_not_rounded_via_f64() {
    let fixture = Fixture::new();
    let path = fixture.path("numeric.fdxproj");
    legacy_database(
        &fixture,
        &path,
        br#"{"Donnees!2:2":{"action":"set","value":1234567890.123456789012345}}"#,
    );
    let loaded = open_project(&path, &fixture.path("cache")).unwrap();
    assert_eq!(
        loaded.decisions["Donnees!2:2"].value,
        CellValue::typed("decimal", "1234567890.123456789012345")
    );
}

#[test]
fn v2_project_roundtrip_is_atomic_and_preserves_typed_values_and_ui_state() {
    let fixture = Fixture::new();
    let mut project = SavedProject {
        config: fixture.config.clone(),
        decisions: BTreeMap::from([(
            cell_key("Donnees", 2, 1),
            Decision {
                action: "set".into(),
                value: CellValue::text("001234567890123456789"),
                ..Default::default()
            },
        )]),
        history: vec![
            json!({"revision": 1, "decisions": {"Donnees!2:2": {"action":"set", "value":{"kind":"decimal","value":"1.234567890123456789"}}}}),
        ],
        ui_state: json!({"mode":"advanced","selection":["Donnees!2:1"]}),
    };
    let path = fixture.path("saved.fdxproj");
    save_project(&path, &project).unwrap();
    let loaded = open_project(&path, &fixture.path("cache")).unwrap();
    assert_eq!(loaded.decisions, project.decisions);
    assert_eq!(loaded.history, project.history);
    assert_eq!(loaded.ui_state, project.ui_state);
    project.history.push(json!({"action":"keep"}));
    save_project(&path, &project).unwrap();
    assert_eq!(
        open_project(&path, &fixture.path("cache2"))
            .unwrap()
            .history
            .len(),
        2
    );
    no_temporary_files(fixture.dir.path());
}

#[test]
fn profiles_exclude_paths_reset_confirmation_and_preserve_decimal_bounds() {
    let mut fixture = Fixture::new();
    fixture.config.fields[1].minimum = Some("1234567890.1234567890123456789".into());
    let path = fixture.path("profile.json");
    save_profile(&path, &fixture.config).unwrap();
    let text = fs::read_to_string(&path).unwrap();
    assert!(!text.contains(&fixture.config.source_path));
    assert!(!text.contains("Donnees"));
    let loaded = load_profile(&path, &fixture.config).unwrap();
    assert_eq!(loaded.source_path, fixture.config.source_path);
    assert_eq!(loaded.sheet, fixture.config.sheet);
    assert!(loaded.mappings.iter().all(|m| !m.confirmed));
    assert_eq!(loaded.fields[1].minimum, fixture.config.fields[1].minimum);
    save_profile(&path, &fixture.config).unwrap();
    no_temporary_files(fixture.dir.path());
}

#[test]
fn legacy_profile_import_is_non_destructive_and_defaults_reports() {
    let fixture = Fixture::new();
    let path = fixture.path("legacy.json");
    fs::write(&path, br#"{"version":1,"source_path":"untrusted.xlsx","sheet":"untrusted","fields":[{"code":"M","label":"Montant","minimum":1234567890.123456789012345}],"mappings":[{"column":2,"field_code":"M","confirmed":true}],"chronology":[{"before":"A","after":"B"}]}"#).unwrap();
    let before = fs::read(&path).unwrap();
    let loaded = load_profile(&path, &fixture.config).unwrap();
    assert_eq!(
        loaded.fields[0].minimum.as_deref(),
        Some("1234567890.123456789012345")
    );
    assert_eq!(loaded.version, 2);
    assert_eq!(loaded.reports, ["control", "original", "verify"]);
    assert!(loaded.chronology[0].enabled);
    assert!(!loaded.mappings[0].confirmed);
    assert!(save_profile(&path, &loaded).is_err());
    assert_eq!(fs::read(&path).unwrap(), before);
}

#[test]
fn corrupt_project_source_or_dictionary_is_rejected_without_cache_materialization() {
    let fixture = Fixture::new();
    let path = fixture.path("corrupt.fdxproj");
    save_project(
        &path,
        &SavedProject {
            config: fixture.config.clone(),
            ..Default::default()
        },
    )
    .unwrap();
    let db = Connection::open(&path).unwrap();
    db.execute(
        "UPDATE project SET value = 'bad' WHERE key = 'dictionary_hash'",
        [],
    )
    .unwrap();
    drop(db);
    let cache = fixture.path("cache");
    assert!(open_project(&path, &cache).is_err());
    assert!(!cache.exists());
    let legacy = fixture.path("bad-source.fdxproj");
    legacy_database(&fixture, &legacy, b"{}");
    let db = Connection::open(&legacy).unwrap();
    db.execute(
        "UPDATE project SET value = 'bad' WHERE key = 'source_hash'",
        [],
    )
    .unwrap();
    drop(db);
    assert!(open_project(&legacy, &cache).is_err());
    assert!(!cache.exists());
}

#[test]
fn unsupported_versions_and_malicious_sqlite_views_are_refused() {
    let fixture = Fixture::new();
    let path = fixture.path("future.json");
    fs::write(&path, br#"{"version":99}"#).unwrap();
    assert!(load_profile(&path, &fixture.config).is_err());
    let path = fixture.path("view.fdxproj");
    let db = Connection::open(&path).unwrap();
    db.execute(
        "CREATE VIEW project AS SELECT 'version' AS key, '1' AS value",
        [],
    )
    .unwrap();
    drop(db);
    assert!(open_project(&path, &fixture.path("cache")).is_err());
}

#[test]
fn storage_does_not_overwrite_sources_dictionaries_or_arbitrary_existing_files() {
    let mut fixture = Fixture::new();
    let dictionary = fixture.path("dictionary.json");
    fs::write(&dictionary, b"private dictionary").unwrap();
    fixture.config.dictionary_path = dictionary.to_string_lossy().into_owned();
    assert!(save_profile(&dictionary, &fixture.config).is_err());
    assert_eq!(fs::read(&dictionary).unwrap(), b"private dictionary");
    let project = SavedProject {
        config: fixture.config.clone(),
        ..Default::default()
    };
    assert!(save_project(&fixture.source, &project).is_err());
    let path = fixture.path("existing.fdxproj");
    fs::write(&path, b"not a project").unwrap();
    assert!(save_project(&path, &project).is_err());
    assert_eq!(fs::read(&path).unwrap(), b"not a project");
}

#[test]
fn legacy_non_midnight_dates_are_refused_and_history_numbers_remain_exact() {
    let fixture = Fixture::new();
    let path = fixture.path("time.fdxproj");
    legacy_database(
        &fixture,
        &path,
        br#"{"Donnees!2:1":{"action":"set","value":"2025-09-30T12:34:56","value_kind":"date"}}"#,
    );
    assert!(open_project(&path, &fixture.path("cache")).is_err());
    let path = fixture.path("history.fdxproj");
    legacy_database(&fixture, &path, b"{}");
    let db = Connection::open(&path).unwrap();
    db.execute(
        "UPDATE project SET value = ?1 WHERE key = 'history'",
        ["[{\"legacy_number\":1.234567890123456789}]"],
    )
    .unwrap();
    drop(db);
    let loaded = open_project(&path, &fixture.path("cache")).unwrap();
    assert_eq!(
        loaded.history[0]["legacy_number"].to_string(),
        "1.234567890123456789"
    );
    let migrated = fixture.path("history-v2.fdxproj");
    save_project(&migrated, &loaded).unwrap();
    let reopened = open_project(&migrated, &fixture.path("cache2")).unwrap();
    assert_eq!(reopened.history, loaded.history);
}
