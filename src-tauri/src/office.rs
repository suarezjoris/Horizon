use docx_rs::*;
use rust_xlsxwriter::{Workbook, Format, Color};
use std::path::PathBuf;
use crate::settings;
use serde::Deserialize;
use genpdf::{Document, Element, fonts, style};
use genpdf::elements::{Paragraph as PdfParagraph, Break, TableLayout};

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum DocxElement {
    Paragraph { 
        text: String, 
        #[serde(default)] bold: bool, 
        #[serde(default)] italic: bool,
        #[serde(default)] align: String // "left", "center", "right", "both"
    },
    Heading { 
        text: String, 
        level: usize 
    },
    List {
        items: Vec<String>,
    },
    Metadata { 
        label: String, 
        value: String 
    }
}

#[derive(Deserialize)]
pub struct DocxContent {
    pub filename: String,
    pub title: String,
    pub elements: Vec<DocxElement>,
    pub template: Option<String>,
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

#[derive(Deserialize, serde::Serialize)]
pub struct PptxContent {
    pub filename: String,
    pub title: String,
    pub slides: Vec<PptxSlide>,
    pub template: Option<String>,
}

#[derive(Deserialize, serde::Serialize)]
pub struct PptxSlide {
    pub title: String,
    pub intro: String,
    pub bullets: Vec<String>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
pub struct PdfContent {
    pub filename: String,
    pub title: String,
    pub elements: Vec<PdfElement>,
    pub template: Option<String>,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum PdfElement {
    Heading { level: u8, text: String },
    Paragraph { text: String, bold: Option<bool>, italic: Option<bool> },
    Table { headers: Vec<String>, rows: Vec<Vec<String>> },
    Image { path: String, caption: Option<String> },
    PageBreak,
    List { items: Vec<String>, ordered: Option<bool> },
}

#[tauri::command]
pub async fn generate_docx(content: DocxContent) -> Result<String, String> {
    let s = settings::load();
    let mut path = PathBuf::from(&s.vault_path).join("documents");
    std::fs::create_dir_all(&path).map_err(|e| e.to_string())?;
    
    // SECURITY FIX (Vuln 4): Extract only the valid file name to prevent path traversal
    let safe_filename = std::path::Path::new(&content.filename)
        .file_name()
        .unwrap_or(std::ffi::OsStr::new("document"))
        .to_string_lossy()
        .into_owned();
    
    let filename = if safe_filename.ends_with(".docx") { safe_filename } else { format!("{}.docx", safe_filename) };
    path.push(&filename);

    let mut doc = if let Some(tpl_name) = &content.template {
        let tpl_path = PathBuf::from(&s.vault_path).join("templates/docx").join(format!("{}.docx", tpl_name));
        if tpl_path.exists() {
            if let Ok(buf) = std::fs::read(&tpl_path) {
                if let Ok(loaded) = read_docx(&buf) {
                    loaded
                } else {
                    Docx::new()
                }
            } else {
                Docx::new()
            }
        } else {
            Docx::new()
        }
    } else {
        Docx::new()
    };

    // Only add the main title if we didn't load from a template, 
    // or if you want to always prepend it
    doc = doc.add_paragraph(Paragraph::new()
        .add_run(Run::new()
            .add_text(&content.title)
            .bold()
            .size(48)
            .color("2E74B5")
            .fonts(RunFonts::new().ascii("Calibri")))
        .align(AlignmentType::Center)
        .line_spacing(LineSpacing::new().after(600)));

    for el in content.elements {
        match el {
            DocxElement::Heading { text, level } => {
                let size = if level == 1 { 36 } else { 28 };
                doc = doc.add_paragraph(Paragraph::new()
                    .add_run(Run::new()
                        .add_text(text)
                        .bold()
                        .size(size)
                        .fonts(RunFonts::new().ascii("Calibri")))
                    .line_spacing(LineSpacing::new().before(400).after(200)));
            },
            DocxElement::Paragraph { text, bold, italic, align } => {
                let mut run = Run::new()
                    .add_text(text)
                    .size(22)
                    .fonts(RunFonts::new().ascii("Calibri"));
                if bold { run = run.bold(); }
                if italic { run = run.italic(); }
                
                let alignment = match align.as_str() {
                    "center" => AlignmentType::Center,
                    "right" => AlignmentType::Right,
                    "both" => AlignmentType::Both,
                    _ => AlignmentType::Left,
                };

                doc = doc.add_paragraph(Paragraph::new()
                    .add_run(run)
                    .align(alignment)
                    .line_spacing(LineSpacing::new().after(200)));
            },
            DocxElement::List { items } => {
                for item in items {
                    doc = doc.add_paragraph(Paragraph::new()
                        .add_run(Run::new().add_text(format!("• {}", item)).size(22))
                        .indent(Some(420), None, None, None)
                        .line_spacing(LineSpacing::new().after(100)));
                }
            },
            DocxElement::Metadata { label, value } => {
                doc = doc.add_paragraph(Paragraph::new()
                    .add_run(Run::new().add_text(format!("{}: ", label)).bold().size(22))
                    .add_run(Run::new().add_text(value).size(22))
                    .line_spacing(LineSpacing::new().after(80)));
            }
        }
    }

    // Add professional footer
    doc = doc.add_paragraph(Paragraph::new()
        .add_run(Run::new()
            .add_text("________________________________________________")
            .color("CCCCCC"))
        .align(AlignmentType::Center)
        .line_spacing(LineSpacing::new().before(400)));

    doc = doc.add_paragraph(Paragraph::new()
        .add_run(Run::new()
            .add_text("Generated by Horizon AI — Premium Intelligence Engine")
            .italic()
            .size(16)
            .color("888888"))
        .align(AlignmentType::Center));

    let file = std::fs::File::create(&path).map_err(|e| e.to_string())?;
    doc.build().pack(file).map_err(|e| e.to_string())?;

    Ok(path.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn generate_pptx(content: PptxContent) -> Result<String, String> {
    let s = settings::load();
    let base_path = PathBuf::from(&s.vault_path).join("documents");
    std::fs::create_dir_all(&base_path).map_err(|e| e.to_string())?;

    // SECURITY FIX (Vuln 4)
    let safe_filename = std::path::Path::new(&content.filename)
        .file_name()
        .unwrap_or(std::ffi::OsStr::new("presentation"))
        .to_string_lossy()
        .into_owned();

    let filename = if safe_filename.ends_with(".pptx") { safe_filename } else { format!("{}.pptx", safe_filename) };
    let output_path = base_path.join(&filename);
    
    let mut data = serde_json::to_value(&content).map_err(|e| e.to_string())?;
    data["output_path"] = serde_json::json!(output_path.to_string_lossy());
    
    // Check for master template (priority: arg > assets > documents)
    let explicit_template = content.template.as_ref().map(|t| PathBuf::from(&s.vault_path).join("templates/pptx").join(format!("{}.pptx", t)));
    let master_template = PathBuf::from(&s.vault_path).join("assets/template.pptx");
    let user_template = base_path.join("template.pptx");
    
    if let Some(et) = explicit_template {
        if et.exists() {
            data["template_path"] = serde_json::json!(et.to_string_lossy());
        }
    } else if master_template.exists() {
        data["template_path"] = serde_json::json!(master_template.to_string_lossy());
    } else if user_template.exists() {
        data["template_path"] = serde_json::json!(user_template.to_string_lossy());
    }

    let home = dirs::home_dir().ok_or("Could not find home directory")?;
    let project_root = home.join("Projects/Horizon");
    
    let python_path = project_root.join(".venv/bin/python3");
    let script_path = project_root.join("src-tauri/src/pptx_gen.py");

    let output = std::process::Command::new(python_path)
        .arg(script_path)
        .arg(serde_json::to_string(&data).unwrap())
        .output()
        .map_err(|e| format!("Failed to run PPTX generator: {}", e))?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).to_string());
    }

    Ok(output_path.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn generate_xlsx(content: XlsxContent) -> Result<String, String> {
    let s = settings::load();
    let mut path = PathBuf::from(&s.vault_path).join("documents");
    std::fs::create_dir_all(&path).map_err(|e| e.to_string())?;

    // SECURITY FIX (Vuln 4)
    let safe_filename = std::path::Path::new(&content.filename)
        .file_name()
        .unwrap_or(std::ffi::OsStr::new("spreadsheet"))
        .to_string_lossy()
        .into_owned();

    let filename = if safe_filename.ends_with(".xlsx") { safe_filename } else { format!("{}.xlsx", safe_filename) };
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

#[tauri::command]
pub async fn generate_pdf(content: PdfContent) -> Result<String, String> {
    let s = settings::load();
    let mut path = PathBuf::from(&s.vault_path).join("documents");
    std::fs::create_dir_all(&path).map_err(|e| e.to_string())?;

    let safe_filename = std::path::Path::new(&content.filename)
        .file_name()
        .unwrap_or(std::ffi::OsStr::new("document"))
        .to_string_lossy()
        .into_owned();

    let filename = if safe_filename.ends_with(".pdf") { safe_filename } else { format!("{}.pdf", safe_filename) };
    path.push(&filename);

    let font_dir = std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("src/fonts");
    
    let font_family = match fonts::from_files(&font_dir, "Inter", None) {
        Ok(f) => f,
        Err(_) => return Err("Could not load fonts from src/fonts".to_string()),
    };

    let mut doc = Document::new(font_family);
    doc.set_title(&content.title);

    doc.push(PdfParagraph::new(&content.title).styled(style::Style::new().bold().with_font_size(24)));
    doc.push(Break::new(1));

    for element in content.elements {
        match element {
            PdfElement::Heading { level, text } => {
                let size = match level { 1 => 20, 2 => 16, 3 => 14, _ => 12 };
                doc.push(PdfParagraph::new(text).styled(style::Style::new().bold().with_font_size(size)));
            }
            PdfElement::Paragraph { text, bold, italic } => {
                let mut style = style::Style::new().with_font_size(12);
                if bold.unwrap_or(false) { style = style.bold(); }
                if italic.unwrap_or(false) { style = style.italic(); }
                doc.push(PdfParagraph::new(text).styled(style));
            }
            PdfElement::Table { headers, rows } => {
                let mut table = TableLayout::new(vec![1; headers.len()]);
                let mut header_row = table.row();
                for h in headers {
                    header_row.push_element(PdfParagraph::new(h).styled(style::Style::new().bold()));
                }
                header_row.push().map_err(|e| e.to_string())?;
                
                for r in rows {
                    let mut data_row = table.row();
                    for cell in r {
                        data_row.push_element(PdfParagraph::new(cell));
                    }
                    data_row.push().map_err(|e| e.to_string())?;
                }
                doc.push(table);
            }
            PdfElement::Image { path: _, caption: _ } => {
                doc.push(PdfParagraph::new("[Image Placeholder]").styled(style::Style::new().italic()));
            }
            PdfElement::PageBreak => {
                doc.push(Break::new(1));
            }
            PdfElement::List { items, ordered: _ } => {
                for item in items {
                    doc.push(PdfParagraph::new(format!("• {}", item)));
                }
            }
        }
    }

    doc.render_to_file(&path).map_err(|e| e.to_string())?;

    Ok(path.to_string_lossy().to_string())
}
