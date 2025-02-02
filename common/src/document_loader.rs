use anyhow::{Context, Result};
use std::path::Path;
// use docx::document::Document as DocxDocument;
// use epub::doc::EpubDoc;
// use calamine::{Reader, Xlsx, open_workbook};
use std::fs::File;
use std::io::Read;

pub enum DocumentType {
    PDF,
    //DOCX,
    TXT,
    //EPUB,
    CSV,
    XLSX,
    HTML,
    MD,
}

pub struct DocumentLoader;

impl DocumentLoader {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Vec<String>> {
        let extension = path.as_ref()
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .unwrap_or_default();

        match extension.as_str() {
            "pdf" => Self::load_pdf(path),
            "txt" => Self::load_txt(path),
            _ => Err(anyhow::anyhow!("Unsupported file type: {}", extension))
        }
    }

    fn load_pdf<P: AsRef<Path>>(path: P) -> Result<Vec<String>> {
        use rig::loaders::PdfFileLoader;
        PdfFileLoader::with_glob(path.as_ref().to_str().unwrap())?
            .read()
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .context("Failed to read PDF")
    }

    fn load_txt<P: AsRef<Path>>(path: P) -> Result<Vec<String>> {
        let mut content = String::new();
        File::open(path)?.read_to_string(&mut content)?;
        Ok(vec![content])
    }
}