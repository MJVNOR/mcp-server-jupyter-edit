use serde::{Deserialize, Deserializer, Serialize, Serializer};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notebook {
    pub nbformat: u32,
    #[serde(rename = "nbformat_minor")]
    pub nbformat_minor: u32,
    pub metadata: NotebookMetadata,
    pub cells: Vec<Cell>,
}

impl Default for Notebook {
    fn default() -> Self {
        Self {
            nbformat: 4,
            nbformat_minor: 5,
            metadata: NotebookMetadata::default(),
            cells: Vec::new(),
        }
    }
}

impl Notebook {
    pub fn new() -> Self {
        Self::default()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NotebookMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kernelspec: Option<KernelSpec>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language_info: Option<LanguageInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warnings: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KernelSpec {
    pub name: String,
    pub display_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageInfo {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_extension: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mimetype: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cell {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(rename = "cell_type")]
    pub cell_type: CellType,
    pub source: CellSource,
    pub metadata: CellMetadata,
    pub outputs: Option<Vec<Output>>,
    pub execution_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attachments: Option<serde_json::Value>,
}

impl Cell {
    pub fn new(cell_type: CellType, source: impl Into<String>) -> Self {
        Self {
            id: Some(Self::generate_id()),
            cell_type,
            source: CellSource::Single(source.into()),
            metadata: CellMetadata::default(),
            outputs: None,
            execution_count: None,
            attachments: None,
        }
    }

    pub fn get_or_generate_id(&mut self) -> String {
        if self.id.is_none() {
            self.id = Some(Self::generate_id());
        }
        self.id.clone().unwrap()
    }

    pub fn generate_id() -> String {
        Uuid::new_v4().to_string()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellType {
    Code,
    Markdown,
    Raw,
}

impl Serialize for CellType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            CellType::Code => serializer.serialize_str("code"),
            CellType::Markdown => serializer.serialize_str("markdown"),
            CellType::Raw => serializer.serialize_str("raw"),
        }
    }
}

impl<'de> Deserialize<'de> for CellType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.as_str() {
            "code" => Ok(CellType::Code),
            "markdown" => Ok(CellType::Markdown),
            "raw" => Ok(CellType::Raw),
            _ => Err(serde::de::Error::custom(format!(
                "Unknown cell type: {}",
                s
            ))),
        }
    }
}

impl std::fmt::Display for CellType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CellType::Code => write!(f, "Code"),
            CellType::Markdown => write!(f, "Markdown"),
            CellType::Raw => write!(f, "Raw"),
        }
    }
}

#[derive(Debug, Clone)]
pub enum CellSource {
    Single(String),
    Multi(Vec<String>),
}

impl Serialize for CellSource {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            CellSource::Single(s) => serializer.serialize_str(s),
            CellSource::Multi(lines) => lines.serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for CellSource {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::Error;

        let value = serde_json::Value::deserialize(deserializer)?;
        match value {
            serde_json::Value::String(s) => Ok(CellSource::Single(s)),
            serde_json::Value::Array(arr) => {
                let lines: Result<Vec<String>, _> = arr
                    .into_iter()
                    .map(|v| {
                        v.as_str()
                            .map(String::from)
                            .ok_or_else(|| Error::custom("Expected string in source array"))
                    })
                    .collect();
                Ok(CellSource::Multi(lines?))
            }
            _ => Err(Error::custom("Expected string or array for source")),
        }
    }
}

impl CellSource {
    pub fn as_string(&self) -> String {
        match self {
            CellSource::Single(s) => s.clone(),
            CellSource::Multi(lines) => lines.join(""),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CellMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    #[serde(default)]
    pub collapsed: bool,
    #[serde(default)]
    pub scrolled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jupyter: Option<JupyterMetadata>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JupyterMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_hidden: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outputs_hidden: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "output_type")]
pub enum Output {
    #[serde(rename = "stream")]
    Stream {
        name: String,
        text: CellSource,
    },
    #[serde(rename = "execute_result")]
    ExecuteResult {
        data: serde_json::Value,
        metadata: serde_json::Value,
        execution_count: Option<u32>,
    },
    #[serde(rename = "display_data")]
    DisplayData {
        data: serde_json::Value,
        metadata: serde_json::Value,
    },
    #[serde(rename = "error")]
    Error {
        ename: String,
        evalue: String,
        traceback: Vec<String>,
    },
}
