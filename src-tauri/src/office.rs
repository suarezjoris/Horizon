use docx_rs::*;
use rust_xlsxwriter::{Workbook, Format, Color};
use std::path::PathBuf;
use crate::settings;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct DocxContent {
    pub filename: String,
    pub title: String,
    pub sections: Vec<DocxSection>,
}

#[derive(Deserialize)]
pub struct DocxSection {
    pub heading: String,
    pub body: String,
}

#[derive(Deserialize)]
pub struct XlsxContent {
    pub filename: String,
    pub sheets: Vec<XlsxSheet>,
}

#[derive(Deserialize)]
pub struct XlsxSheet {
    pub name: String,
    pub rows: Vec<Vec<String>>,
}

#[tauri::command]
pub async fn generate_docx(content: DocxContent) -> Result<String, String> {
    let s = settings::load();
    let mut path = PathBuf::from(&s.vault_path).join("documents");
    std::fs::create_dir_all(&path).map_err(|e| e.to_string())?;
    
    let filename = if content.filename.ends_with(".docx") { content.filename } else { format!("{}.docx", content.filename) };
    path.push(&filename);

    let mut doc = Docx::new()
        .add_paragraph(Paragraph::new()
            .add_run(Run::new()
                .add_text(&content.title)
                .bold()
                .size(32))
            .align(AlignmentType::Center));

    for section in content.sections {
        doc = doc.add_paragraph(Paragraph::new()
            .add_run(Run::new()
                .add_text(&section.heading)
                .bold()
                .size(24)));
        
        for line in section.body.split('\n') {
            doc = doc.add_paragraph(Paragraph::new()
                .add_run(Run::new().add_text(line)));
        }
    }

    let file = std::fs::File::create(&path).map_err(|e| e.to_string())?;
    doc.build().pack(file).map_err(|e| e.to_string())?;

    Ok(path.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn generate_xlsx(content: XlsxContent) -> Result<String, String> {
    let s = settings::load();
    let mut path = PathBuf::from(&s.vault_path).join("documents");
    std::fs::create_dir_all(&path).map_err(|e| e.to_string())?;

    let filename = if content.filename.ends_with(".xlsx") { content.filename } else { format!("{}.xlsx", content.filename) };
    path.push(&filename);

    let mut workbook = Workbook::new();
    let header_format = Format::new().set_bold().set_background_color(Color::Silver);

    for sheet_data in content.sheets {
        let worksheet = workbook.add_worksheet();
        if !sheet_data.name.is_empty() {
            let _ = worksheet.set_name(&sheet_data.name);
        }

        for (row_idx, row_data) in sheet_data.rows.iter().enumerate() {
            for (col_idx, cell_data) in row_data.iter().enumerate() {
                if row_idx == 0 {
                    worksheet.write_string_with_format(row_idx as u32, col_idx as u16, cell_data, &header_format).map_err(|e| e.to_string())?;
                } else {
                    worksheet.write_string(row_idx as u32, col_idx as u16, cell_data).map_err(|e| e.to_string())?;
                }
            }
        }
    }

    workbook.save(&path).map_err(|e| e.to_string())?;

    Ok(path.to_string_lossy().to_string())
}
