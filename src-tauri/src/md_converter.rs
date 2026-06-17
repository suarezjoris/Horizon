use pulldown_cmark::{Parser, Event, Tag, TagEnd, Options};
use crate::office::{DocxElement, DocxContent, generate_docx};
use crate::{settings, vault};

pub fn markdown_to_docx_elements(md: &str) -> Vec<DocxElement> {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    
    let parser = Parser::new_ext(md, options);
    let mut elements = Vec::new();
    let mut current_text = String::new();
    let mut in_bold = false;
    let mut in_italic = false;
    let mut list_items = Vec::new();
    let mut _in_list = false;
    
    for event in parser {
        match event {
            Event::Start(Tag::Heading { .. }) => { /* start heading */ }
            Event::End(TagEnd::Heading(level)) => {
                elements.push(DocxElement::Heading { 
                    level: level as usize, 
                    text: current_text.clone() 
                });
                current_text.clear();
            }
            Event::Start(Tag::Paragraph) => { /* start para */ }
            Event::End(TagEnd::Paragraph) => {
                if !current_text.trim().is_empty() {
                    elements.push(DocxElement::Paragraph { 
                        text: current_text.clone(), 
                        bold: in_bold, 
                        italic: in_italic, 
                        align: "left".into() 
                    });
                }
                current_text.clear();
            }
            Event::Start(Tag::Strong) => { in_bold = true; }
            Event::End(TagEnd::Strong) => { in_bold = false; }
            Event::Start(Tag::Emphasis) => { in_italic = true; }
            Event::End(TagEnd::Emphasis) => { in_italic = false; }
            Event::Text(text) => { current_text.push_str(&text); }
            Event::Start(Tag::List(_)) => { 
                _in_list = true;
                list_items.clear(); 
            }
            Event::End(TagEnd::List(_)) => {
                _in_list = false;
                if !list_items.is_empty() {
                    elements.push(DocxElement::List { items: list_items.clone() });
                }
                list_items.clear();
            }
            Event::Start(Tag::Item) => { current_text.clear(); }
            Event::End(TagEnd::Item) => {
                list_items.push(current_text.clone());
                current_text.clear();
            }
            Event::SoftBreak | Event::HardBreak => { current_text.push(' '); }
            _ => {}
        }
    }
    
    if !current_text.trim().is_empty() {
        elements.push(DocxElement::Paragraph { 
            text: current_text.clone(), 
            bold: in_bold, 
            italic: in_italic, 
            align: "left".into() 
        });
    }

    elements
}

#[tauri::command]
pub async fn export_note_as_docx(
    rel_path: String,
    _template: Option<String>,
) -> Result<String, String> {
    let _s = settings::load();
    let md_content = vault::read_note(rel_path.clone())?;
    let elements = markdown_to_docx_elements(&md_content);
    let title = rel_path.trim_end_matches(".md").to_string();
    
    let content = DocxContent { filename: title.clone(), title, elements, template: None };
    // Feature 9 adds template support. For now, just generate standard docx.
    generate_docx(content).await
}

#[tauri::command]
pub async fn export_vault_as_docx(template: Option<String>) -> Result<Vec<String>, String> {
    let notes = vault::list_notes();
    let mut paths = Vec::new();
    for note in notes {
        let path = export_note_as_docx(note, template.clone()).await?;
        paths.push(path);
    }
    Ok(paths)
}

