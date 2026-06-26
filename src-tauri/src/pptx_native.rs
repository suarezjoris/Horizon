use crate::office::PptxContent;
use std::path::PathBuf;
use std::io::{Read, Write, Cursor};
use zip::{ZipArchive, ZipWriter, write::FileOptions};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct PptxTemplate {
    pub id: String,
    pub title: String,
    pub thumbnail_url: String,
    pub download_url: String,
    pub source: String,
}

#[tauri::command]
pub async fn analyze_pptx_request(prompt: String) -> Result<String, String> {
    let system_prompt = "Tu es un directeur artistique. Extrait 1 ou 2 mots-clés représentant le thème visuel idéal pour cette demande de présentation. Réponds UNIQUEMENT par ces mots-clés (ex: 'finance', 'startup', 'nature'), séparés par un espace, sans ponctuation.";
    let messages = vec![
        serde_json::json!({ "role": "system", "content": system_prompt }),
        serde_json::json!({ "role": "user", "content": prompt })
    ];
    let keywords = crate::ollama::chat_once(messages, "qwen2.5-coder:32b").await?;
    Ok(keywords.trim().to_string())
}

#[tauri::command]
pub async fn execute_pptx_generation(prompt: String, template_url: String) -> Result<String, String> {
    let system_prompt = r#"Tu es un expert en présentation. Crée une présentation détaillée basée sur la demande de l'utilisateur.
Tu dois répondre STRICTEMENT avec un objet JSON au format suivant, sans AUCUN texte avant ou après:
{
  "filename": "ma_super_presentation",
  "title": "Titre principal de la présentation",
  "slides": [
    {
      "title": "Titre de la slide",
      "intro": "Texte d'introduction de la slide",
      "bullets": ["Point 1", "Point 2"]
    }
  ]
}
Génère le nombre de slides demandé par l'utilisateur."#;

    let messages = vec![
        serde_json::json!({ "role": "system", "content": system_prompt }),
        serde_json::json!({ "role": "user", "content": prompt })
    ];
    let json_response = crate::ollama::chat_once_json(messages, "qwen2.5-coder:32b").await?;
    
    // Robust JSON extraction
    let start_idx = json_response.find('{');
    let end_idx = json_response.rfind('}');
    let cleaned = if let (Some(start), Some(end)) = (start_idx, end_idx) {
        if start <= end {
            &json_response[start..=end]
        } else {
            &json_response
        }
    } else {
        &json_response
    };
    
    let content: PptxContent = serde_json::from_str(cleaned).map_err(|e| format!("Erreur JSON Ollama: {}\nRéponse: {}", e, cleaned))?;
    
    let settings = crate::settings::load();
    let base_path = std::path::PathBuf::from(&settings.vault_path).join("documents");
    std::fs::create_dir_all(&base_path).map_err(|e| e.to_string())?;
    
    let filename = format!("{}.pptx", content.filename);
    let output_path = base_path.join(&filename);
    
    let mut template_path = None;
    if template_url.starts_with("http") {
        if let Ok(res) = reqwest::get(&template_url).await {
            if let Ok(bytes) = res.bytes().await {
                // Vérifier les magic bytes d'un fichier ZIP (PK\x03\x04)
                if bytes.len() > 4 && bytes[0] == b'P' && bytes[1] == b'K' {
                    let temp_dir = std::env::temp_dir();
                    let tpl_path = temp_dir.join("downloaded_template.pptx");
                    if std::fs::write(&tpl_path, bytes).is_ok() {
                        template_path = Some(tpl_path);
                    }
                }
            }
        }
    }

    // Destruction du fallback local : on veut du dynamique ou on crash.
    if template_path.is_none() {
        return Err(format!("Le fichier template téléchargé est invalide ou n'est pas un fichier ZIP (PPTX). URL tentée : {}", template_url));
    }

    generate_pptx_native_sync(&content, template_path, output_path)
}

#[derive(Deserialize)]
struct GitHubCodeSearchResponse {
    items: Vec<GitHubCodeItem>,
}

#[derive(Deserialize)]
struct GitHubCodeItem {
    name: String,
    html_url: String,
    repository: GitHubRepository,
}

#[derive(Deserialize)]
struct GitHubRepository {
    full_name: String,
    owner: GitHubOwner,
}

#[derive(Deserialize)]
struct GitHubOwner {
    avatar_url: String,
}

#[tauri::command]
pub async fn scrape_pptx_templates(query: String) -> Result<Vec<PptxTemplate>, String> {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(reqwest::header::USER_AGENT, reqwest::header::HeaderValue::from_static("Horizon-App/1.0"));
    headers.insert(reqwest::header::ACCEPT, reqwest::header::HeaderValue::from_static("application/vnd.github.v3+json"));
    
    if let Ok(token) = std::env::var("GITHUB_TOKEN") {
        if let Ok(auth_val) = reqwest::header::HeaderValue::from_str(&format!("Bearer {}", token)) {
            headers.insert(reqwest::header::AUTHORIZATION, auth_val);
        }
    }

    let client = reqwest::Client::builder()
        .default_headers(headers)
        .build()
        .map_err(|e| format!("Reqwest error: {}", e))?;
        
    let search_query = format!("extension:pptx {}", query);
    let url = format!("https://api.github.com/search/code?q={}&per_page=4", urlencoding::encode(&search_query));
    
    let mut templates = Vec::new();
    
    if let Ok(res) = client.get(&url).send().await {
        if res.status().is_success() {
            if let Ok(json) = res.json::<GitHubCodeSearchResponse>().await {
                for item in json.items {
                    // Convert github.com/owner/repo/blob/branch/path to github.com/owner/repo/raw/branch/path
                    // This bypasses the HTML viewer and serves the raw file directly, regardless of default branch.
                    let raw_download_url = item.html_url.replace("/blob/", "/raw/");
                    
                    templates.push(PptxTemplate {
                        id: urlencoding::encode(&item.name).into_owned(),
                        title: item.name.replace(".pptx", "").replace("-", " ").replace("_", " "),
                        thumbnail_url: item.repository.owner.avatar_url,
                        download_url: raw_download_url,
                        source: item.repository.full_name,
                    });
                }
            } else {
                return Err("Failed to parse GitHub JSON response".to_string());
            }
        } else {
            return Err(format!("GitHub API Error: {}", res.status()));
        }
    } else {
        return Err("Failed to connect to GitHub API".to_string());
    }
    
    if templates.is_empty() {
        return Err("Aucun template trouvé dynamiquement pour cette recherche sur GitHub.".into());
    }
    
    Ok(templates)
}

pub fn generate_pptx_native_sync(content: &PptxContent, template_path: Option<PathBuf>, output_path: PathBuf) -> Result<String, String> {
    // If we have an actual template file, we modify it.
    // For the sake of the exercise (and because a full valid PPTX is complex), 
    // we'll implement the XML text replacement and duplication logic using basic string ops / quick-xml 
    // which operates extremely fast in memory.
    
    let template_bytes = if let Some(path) = template_path {
        if path.exists() {
            std::fs::read(&path).map_err(|e| format!("Failed to read template: {}", e))?
        } else {
            create_empty_pptx_bytes()?
        }
    } else {
        create_empty_pptx_bytes()?
    };

    let cursor = Cursor::new(template_bytes);
    let mut archive = ZipArchive::new(cursor).map_err(|e| format!("Zip error: {}", e))?;
    
    let mut out_buf = Cursor::new(Vec::new());
    {
        let mut zip_writer = ZipWriter::new(&mut out_buf);
        let options: FileOptions<'_, ()> = FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        
        let slide_count = content.slides.len();
        
        // We'll track the original slide2.xml content to duplicate it
        let mut base_slide_xml = String::new();
        let mut base_slide_rels = String::new();
        
        // Extract base slide (slide2 usually, or slide1)
        for i in 0..archive.len() {
            let mut file = archive.by_index(i).unwrap();
            if file.name() == "ppt/slides/slide1.xml" {
                file.read_to_string(&mut base_slide_xml).unwrap();
            } else if file.name() == "ppt/slides/_rels/slide1.xml.rels" {
                file.read_to_string(&mut base_slide_rels).unwrap();
            }
        }
        
        if base_slide_xml.is_empty() {
            base_slide_xml = "<p:sld xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" xmlns:p=\"http://schemas.openxmlformats.org/presentationml/2006/main\"><p:cSld><p:spTree><p:sp><p:txBody><a:p><a:r><a:t>TITLE_PLACEHOLDER</a:t></a:r></a:p></p:txBody></p:sp><p:sp><p:txBody><a:p><a:r><a:t>CONTENT_PLACEHOLDER</a:t></a:r></a:p></p:txBody></p:sp></p:spTree></p:cSld></p:sld>".to_string();
        }

        // Process all existing files in the zip
        for i in 0..archive.len() {
            let mut file = archive.by_index(i).unwrap();
            let name = file.name().to_string();
            
            // We'll intercept and modify specific files
            let mut content_buf = Vec::new();
            file.read_to_end(&mut content_buf).unwrap();
            
            if name == "[Content_Types].xml" {
                let mut xml = String::from_utf8_lossy(&content_buf).to_string();
                // Add entries for new slides
                for s in 0..slide_count {
                    let slide_idx = s + 1;
                    if !xml.contains(&format!("PartName=\"/ppt/slides/slide{}.xml\"", slide_idx)) {
                        let insert_idx = xml.rfind("</Types>").unwrap_or(xml.len());
                        xml.insert_str(insert_idx, &format!("<Override PartName=\"/ppt/slides/slide{}.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.presentationml.slide+xml\"/>", slide_idx));
                    }
                }
                zip_writer.start_file(&name, options).unwrap();
                zip_writer.write_all(xml.as_bytes()).unwrap();
                
            } else if name == "ppt/_rels/presentation.xml.rels" {
                let mut xml = String::from_utf8_lossy(&content_buf).to_string();
                for s in 0..slide_count {
                    let slide_idx = s + 1;
                    let r_id = format!("rIdSlide{}", slide_idx);
                    if !xml.contains(&r_id) {
                        let insert_idx = xml.rfind("</Relationships>").unwrap_or(xml.len());
                        xml.insert_str(insert_idx, &format!("<Relationship Id=\"{}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide\" Target=\"slides/slide{}.xml\"/>", r_id, slide_idx));
                    }
                }
                zip_writer.start_file(&name, options).unwrap();
                zip_writer.write_all(xml.as_bytes()).unwrap();
                
            } else if name == "ppt/presentation.xml" {
                let mut xml = String::from_utf8_lossy(&content_buf).to_string();
                let mut slide_id_lst = String::new();
                for s in 0..slide_count {
                    let slide_idx = s + 1;
                    let r_id = format!("rIdSlide{}", slide_idx);
                    let s_id = 255 + slide_idx; // typical slide IDs start > 255
                    slide_id_lst.push_str(&format!("<p:sldId id=\"{}\" r:id=\"{}\"/>", s_id, r_id));
                }
                
                if let Some(start) = xml.find("<p:sldIdLst>") {
                    if let Some(end) = xml[start..].find("</p:sldIdLst>") {
                        let absolute_end = start + end;
                        xml.replace_range(start + 12..absolute_end, &slide_id_lst);
                    }
                } else if let Some(insert_idx) = xml.find("</p:presentation>") {
                    xml.insert_str(insert_idx, &format!("<p:sldIdLst>{}</p:sldIdLst>", slide_id_lst));
                }
                
                zip_writer.start_file(&name, options).unwrap();
                zip_writer.write_all(xml.as_bytes()).unwrap();
                
            } else if name.starts_with("ppt/slides/slide") && name.ends_with(".xml") {
                // Skip original slides, we will generate them
                continue;
            } else if name.starts_with("ppt/slides/_rels/slide") && name.ends_with(".xml.rels") {
                // Skip original rels, we will generate them
                continue;
            } else {
                zip_writer.start_file(&name, options).unwrap();
                zip_writer.write_all(&content_buf).unwrap();
            }
        }
        
        // Generate the 25 slides
        for s in 0..slide_count {
            let slide_idx = s + 1;
            let slide_data = &content.slides[s];
            
            // 1. Create slide XML
            let mut new_slide_xml = base_slide_xml.clone();
            
            // Very naive text replacement for proof of concept!
            // Real implementation would parse the XML with quick-xml and replace <a:t> inner text properly.
            // We use simple string replacement of common placeholder tokens or the first text blocks.
            // For the title slide (s == 0), we use the presentation title.
            let display_title = if s == 0 { &content.title } else { &slide_data.title };
            
            // Replace text tags containing generic text
            new_slide_xml = new_slide_xml.replacen("TITLE_PLACEHOLDER", display_title, 1);
            
            let mut content_text = format!("{}\n", slide_data.intro);
            for bullet in &slide_data.bullets {
                content_text.push_str(&format!("• {}\n", bullet));
            }
            new_slide_xml = new_slide_xml.replacen("CONTENT_PLACEHOLDER", &content_text, 1);
            
            zip_writer.start_file(format!("ppt/slides/slide{}.xml", slide_idx), options).unwrap();
            zip_writer.write_all(new_slide_xml.as_bytes()).unwrap();
            
            // 2. Create slide Rels
            if !base_slide_rels.is_empty() {
                zip_writer.start_file(format!("ppt/slides/_rels/slide{}.xml.rels", slide_idx), options).unwrap();
                zip_writer.write_all(base_slide_rels.as_bytes()).unwrap();
            }
        }
        
        zip_writer.finish().map_err(|e| format!("Finish zip error: {}", e))?;
    }
    
    std::fs::write(&output_path, out_buf.into_inner()).map_err(|e| format!("Write file error: {}", e))?;
    
    Ok(output_path.to_string_lossy().to_string())
}

// Minimal valid PPTX bytes if no template is found
fn create_empty_pptx_bytes() -> Result<Vec<u8>, String> {
    let hardcoded = std::path::PathBuf::from("/home/joris/Projects/Horizon/vault/templates/pptx/professional.pptx");
    if hardcoded.exists() {
        if let Ok(bytes) = std::fs::read(&hardcoded) {
            return Ok(bytes);
        }
    }
    Err("Aucun template local n'a été trouvé. Veuillez placer un fichier 'professional.pptx' dans votre dossier 'vault/templates/pptx/'. Le fichier téléchargé n'était pas un ZIP valide.".into())
}
