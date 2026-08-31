pub mod base64_util;
pub mod compatibility;
pub mod docx;
pub mod pdf;
pub mod pptx;
pub mod pptx_import;
pub mod xlsx;

pub use docx::{
    export_doc_to_docx, import_docx_to_doc, import_docx_to_doc_with_report, DocxImportResult,
};
pub use compatibility::export_compatibility_warnings;
pub use pdf::{export_deck_to_pdf, export_doc_to_pdf, export_workbook_to_pdf};
pub use pptx::export_deck_to_pptx;
pub use pptx_import::{import_deck_from_pptx_with_report, PptxImportResult};
pub use xlsx::{
    export_workbook_to_xlsx, import_workbook_from_xlsx, import_workbook_from_xlsx_with_report,
    XlsxImportResult,
};

#[cfg(test)]
mod tests {
    use super::*;
    use redoc_sheet_engine::{SheetCell, WorkbookModel};
    use redoc_slide_engine::{DeckModel, ElementKind, SlideElement};
    use std::io::Read;

    #[test]
    fn test_xlsx_round_trip() {
        let mut workbook = WorkbookModel::new_default();
        workbook.sheets[0].cells.insert(
            "1:1".to_string(),
            SheetCell {
                raw_value: "100".to_string(),
                display_value: "100".to_string(),
                formula: None,
                style: None,
            },
        );
        let path =
            std::env::temp_dir().join(format!("redoc_xlsx_test_{}.xlsx", std::process::id()));
        let bytes = export_workbook_to_xlsx(&workbook).expect("Failed to export xlsx");
        std::fs::write(&path, bytes).expect("Failed to write xlsx file");

        let imported = import_workbook_from_xlsx(&path).expect("Failed to import xlsx");
        assert_eq!(imported.sheets[0].cells["1:1"].raw_value, "100");

        std::fs::remove_file(path).expect("Failed to clean up xlsx file");
    }

    #[test]
    fn test_pptx_xml_structure() {
        let mut deck = DeckModel::new_default();
        deck.slides[0].elements.push(SlideElement {
            id: "text-1".to_string(),
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 50.0,
            rotation: 0.0,
            z_index: 1,
            entrance: "none".to_string(),
            kind: ElementKind::Text {
                text: "Hello PPTX".to_string(),
                font_size: 24.0,
                font_family: "Arial".to_string(),
                color: "#000000".to_string(),
                align: "left".to_string(),
                bold: true,
                italic: false,
                underline: false,
                bullets: false,
            },
        });

        let bytes = export_deck_to_pptx(&deck).expect("Failed to export pptx");
        let mut archive =
            zip::ZipArchive::new(std::io::Cursor::new(bytes)).expect("Failed to open pptx zip");

        let mut presentation_xml = String::new();
        archive
            .by_name("ppt/presentation.xml")
            .expect("Missing presentation.xml")
            .read_to_string(&mut presentation_xml)
            .expect("Failed to read presentation.xml");
        assert!(presentation_xml.contains("<p:presentation"));

        let mut slide_xml = String::new();
        archive
            .by_name("ppt/slides/slide1.xml")
            .expect("Missing slide1.xml")
            .read_to_string(&mut slide_xml)
            .expect("Failed to read slide1.xml");
        assert!(slide_xml.contains("Hello PPTX"));
        assert!(slide_xml.contains(r#"b="1""#));
    }
}
