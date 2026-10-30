use crate::models::*;
use bigdecimal::{BigDecimal, ToPrimitive};
use chrono::{Datelike, Duration, NaiveDate};
use quick_xml::{events::Event, Reader};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    io::{Cursor, Read, Seek, Write},
    path::{Component, Path, PathBuf},
    str::FromStr,
};
use zip::{write::SimpleFileOptions, CompressionMethod, ZipArchive, ZipWriter};

pub(crate) const MAIN_NS: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
pub(crate) const REL_NS: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
pub(crate) const MAX_FILE: u64 = 256 * 1024 * 1024;
const MAX_PART: u64 = 256 * 1024 * 1024;
const MAX_TOTAL: u64 = 1_000_000_000;
const MAX_ENTRIES: usize = 20_000;
const MAX_NODES: usize = 6_000_000;
const CONTENT_NS: &str = "http://schemas.openxmlformats.org/package/2006/content-types";
const WORKBOOK_CONTENT: &str =
    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml";
const TEMPLATE_CONTENT: &str =
    "application/vnd.openxmlformats-officedocument.spreadsheetml.template.main+xml";
const WORKBOOK_RECOVERY: &str = "Ouvrez une copie dans Excel ou LibreOffice, reparez le classeur si necessaire, puis enregistrez, fermez et rechargez cette copie. Verifiez les donnees et objets conserves avant toute conversion ; ne renommez pas seulement l'extension. La source reste intacte.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WorkbookFormat {
    Workbook,
    Template,
}

impl WorkbookFormat {
    pub fn suffix(self) -> &'static str {
        match self {
            Self::Workbook => "xlsx",
            Self::Template => "xltx",
        }
    }

    fn from_path(path: &Path) -> Result<Self> {
        let suffix = path
            .extension()
            .and_then(|v| v.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        match suffix.as_str() {
            "xlsx" => Ok(Self::Workbook),
            "xltx" => Ok(Self::Template),
            "xlsm" | "xltm" | "xlam" => Err(error("Format avec macros non pris en charge : la conservation du projet VBA n'est pas garantie par cet exporteur. Aucune macro n'est executee. Conservez l'original ; ne creez une copie .xlsx sans macros dans Excel ou LibreOffice que si leur suppression est explicitement acceptable, puis rechargez cette copie.")),
            "xls" | "xlsb" | "xlt" | "xla" => Err(error("Format Excel binaire non pris en charge par l'exporteur conservateur. Dans Excel ou LibreOffice, enregistrez une copie en .xlsx, verifiez qu'aucune donnee, macro ou objet utile n'est perdu, puis rechargez cette copie. Conservez l'original ; aucune conversion automatique n'est effectuee.")),
            _ => Err(error("Format non pris en charge. Seuls les classeurs .xlsx et modeles .xltx OOXML non chiffres et conservables sont acceptes. Dans Excel ou LibreOffice, enregistrez une copie en .xlsx, verifiez les donnees et objets conserves, puis rechargez-la. Conservez l'original ; renommer l'extension ne convertit pas un fichier.")),
        }
    }
}

pub(crate) fn source_error(path: &Path, cause: FdxError) -> FdxError {
    if matches!(cause, FdxError::Cancelled) {
        return cause;
    }
    let name = path
        .file_name()
        .unwrap_or(path.as_os_str())
        .to_string_lossy();
    let action = match &cause {
        FdxError::Io(e) if e.kind() == std::io::ErrorKind::NotFound => {
            "Fichier introuvable : verifiez son emplacement et selectionnez-le de nouveau."
        }
        FdxError::Io(e)
            if e.kind() == std::io::ErrorKind::PermissionDenied
                || (cfg!(windows) && matches!(e.raw_os_error(), Some(32 | 33))) =>
        {
            "Fichier verrouille ou acces refuse : fermez-le dans Excel ou LibreOffice, verifiez les droits de lecture, puis rechargez-le."
        }
        _ => WORKBOOK_RECOVERY,
    };
    error(format!("Lecture du classeur '{name}' impossible : {cause} {action}"))
}

pub(crate) fn read_source_package(path: &Path, job: &JobControl) -> Result<(Vec<u8>, Package)> {
    job.check()?;
    let expected = WorkbookFormat::from_path(path)?;
    let read = || {
        let bytes = read_bounded(path, MAX_FILE, job)?;
        let package = Package::from_bytes(&bytes, job)?;
        let actual = package.format(job)?;
        if actual != expected {
            return Err(error(format!(
                "Extension et contenu incompatibles : le paquet est de type .{}, pas .{}. Utilisez Enregistrer sous dans Excel ou LibreOffice pour creer une copie au format voulu.",
                actual.suffix(), expected.suffix()
            )));
        }
        Ok((bytes, package))
    };
    read().map_err(|e| source_error(path, e))
}

pub(crate) fn io_error(context: &str, e: impl std::fmt::Display) -> FdxError {
    error(format!("{context} : {e}"))
}

pub fn sha256(path: &Path) -> Result<String> {
    hash_file(path, &JobControl::default())
}

pub(crate) fn hash_file(path: &Path, job: &JobControl) -> Result<String> {
    let mut file = File::open(path)?;
    let mut digest = Sha256::new();
    let mut buf = [0; 65_536];
    loop {
        job.check()?;
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        digest.update(&buf[..n]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

pub(crate) fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub(crate) fn read_bounded(path: &Path, limit: u64, job: &JobControl) -> Result<Vec<u8>> {
    let mut file = File::open(path)?;
    if file.metadata()?.len() > limit {
        return Err(error("Fichier trop volumineux."));
    }
    let mut bytes = Vec::new();
    let mut buf = [0; 65_536];
    loop {
        job.check()?;
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        if bytes.len() as u64 + n as u64 > limit {
            return Err(error("Fichier trop volumineux."));
        }
        bytes.extend_from_slice(&buf[..n]);
    }
    Ok(bytes)
}

pub(crate) fn extension(path: &Path, expected: &str) -> Result<()> {
    if !path
        .extension()
        .and_then(|v| v.to_str())
        .is_some_and(|v| v.eq_ignore_ascii_case(expected))
    {
        return Err(error(format!(
            "Le fichier doit avoir l'extension .{expected}."
        )));
    }
    Ok(())
}

fn normalized_path(path: &Path) -> Result<PathBuf> {
    let absolute = if path.is_absolute() {
        path.to_owned()
    } else {
        std::env::current_dir()?.join(path)
    };
    if absolute.exists() {
        return Ok(fs::canonicalize(absolute)?);
    }
    let mut suffix = Vec::new();
    let mut parent = absolute.as_path();
    while !parent.exists() {
        suffix.push(
            parent
                .file_name()
                .ok_or_else(|| error("Chemin de destination invalide."))?
                .to_owned(),
        );
        parent = parent
            .parent()
            .ok_or_else(|| error("Dossier de destination introuvable."))?;
    }
    let mut result = fs::canonicalize(parent)?;
    for part in suffix.iter().rev() {
        result.push(part);
    }
    let mut clean = PathBuf::new();
    for part in result.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => {
                clean.pop();
            }
            other => clean.push(other.as_os_str()),
        }
    }
    Ok(clean)
}

pub(crate) fn protect_paths(destination: &Path, paths: &[&str]) -> Result<()> {
    let target = normalized_path(destination)?;
    for path in paths.iter().filter(|v| !v.is_empty()) {
        let protected = normalized_path(Path::new(path))?;
        let equal = if cfg!(windows) {
            target
                .to_string_lossy()
                .eq_ignore_ascii_case(&protected.to_string_lossy())
        } else {
            target == protected
        };
        if equal {
            return Err(error(
                "Le fichier source ou dictionnaire ne peut pas etre remplace.",
            ));
        }
        // A hard link is also a source alias, even though atomic replacement would not
        // change the other link. Refuse it to keep the source-protection rule explicit.
        #[cfg(unix)]
        if let (Ok(a), Ok(b)) = (fs::metadata(&target), fs::metadata(&protected)) {
            use std::os::unix::fs::MetadataExt;
            if a.dev() == b.dev() && a.ino() == b.ino() {
                return Err(error("La destination est un lien vers un fichier source."));
            }
        }
        #[cfg(windows)]
        if target.exists()
            && protected.exists()
            && windows_identity(&target)? == windows_identity(&protected)?
        {
            return Err(error("La destination est un lien vers un fichier source."));
        }
    }
    Ok(())
}

#[cfg(windows)]
fn windows_identity(path: &Path) -> Result<(u32, u64)> {
    use std::os::windows::io::AsRawHandle;
    #[repr(C)]
    #[derive(Default)]
    struct FileInfo {
        attributes: u32,
        creation: [u32; 2],
        access: [u32; 2],
        write: [u32; 2],
        volume: u32,
        size_high: u32,
        size_low: u32,
        links: u32,
        index_high: u32,
        index_low: u32,
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn GetFileInformationByHandle(
            file: *mut std::ffi::c_void,
            information: *mut FileInfo,
        ) -> i32;
    }
    let file = File::open(path)?;
    let mut info = FileInfo::default();
    // Both pointers are valid for this call and File keeps the OS handle alive.
    if unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut info) } == 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok((
        info.volume,
        (u64::from(info.index_high) << 32) | u64::from(info.index_low),
    ))
}

#[derive(Debug, Clone)]
pub(crate) struct Node {
    pub name: String,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<usize>,
    pub text: String,
    pub start: usize,
    pub open_end: usize,
    pub close_start: usize,
    pub end: usize,
    pub empty: bool,
}
impl Node {
    pub fn local(&self) -> &str {
        self.name.rsplit(':').next().unwrap_or(&self.name)
    }
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }
    pub fn required(&self, name: &str) -> Result<&str> {
        self.attr(name)
            .ok_or_else(|| error(format!("Attribut XML manquant : {name}.")))
    }
    pub fn tag(&self, updates: &[(&str, Option<String>)], empty: bool) -> String {
        let mut attrs = self.attrs.clone();
        for (key, value) in updates {
            attrs.retain(|(k, _)| k != key);
            if let Some(value) = value {
                attrs.push((key.to_string(), value.clone()));
            }
        }
        let mut text = format!("<{}", self.name);
        for (key, value) in attrs {
            text.push_str(&format!(" {key}=\"{}\"", escape(&value)));
        }
        text.push_str(if empty { "/>" } else { ">" });
        text
    }
    pub fn qualified(&self, local: &str) -> String {
        self.name.rsplit_once(':').map_or_else(
            || local.to_string(),
            |(prefix, _)| format!("{prefix}:{local}"),
        )
    }
}

pub(crate) struct Xml {
    pub nodes: Vec<Node>,
    pub root: usize,
}
impl Xml {
    pub fn parse(bytes: &[u8], job: &JobControl) -> Result<Self> {
        let text = std::str::from_utf8(bytes)
            .map_err(|_| error("Seul le XML UTF-8 est pris en charge."))?;
        if text.contains("<!DOCTYPE") || text.contains("<!ENTITY") {
            return Err(error("Declarations XML DTD ou entites interdites."));
        }
        let mut reader = Reader::from_reader(bytes);
        let mut nodes: Vec<Node> = Vec::new();
        let mut stack: Vec<usize> = Vec::new();
        let mut root = None;
        let mut events = 0usize;
        loop {
            events += 1;
            if events.is_multiple_of(4096) {
                job.check()?;
            }
            let start = reader.buffer_position() as usize;
            let event = reader
                .read_event()
                .map_err(|e| io_error("XML invalide", e))?;
            let end = reader.buffer_position() as usize;
            match event {
                Event::Start(ref tag) | Event::Empty(ref tag) => {
                    if stack.len() >= 128 || nodes.len() >= MAX_NODES {
                        return Err(error("Limite de complexite XML depassee."));
                    }
                    let empty = matches!(event, Event::Empty(_));
                    let name = std::str::from_utf8(tag.name().as_ref())
                        .map_err(|e| io_error("Nom XML invalide", e))?
                        .to_string();
                    let mut attrs = Vec::new();
                    for attribute in tag.attributes() {
                        let attribute =
                            attribute.map_err(|e| io_error("Attribut XML invalide", e))?;
                        let key = std::str::from_utf8(attribute.key.as_ref())
                            .map_err(|e| io_error("Attribut XML invalide", e))?
                            .to_string();
                        let value = attribute
                            .decode_and_unescape_value(reader.decoder())
                            .map_err(|e| io_error("Entite XML invalide", e))?
                            .into_owned();
                        if !xml_text_valid(&value) {
                            return Err(error("Caractere XML interdit."));
                        }
                        attrs.push((key, value));
                    }
                    let id = nodes.len();
                    if let Some(parent) = stack.last() {
                        nodes[*parent].children.push(id);
                    } else if root.replace(id).is_some() {
                        return Err(error("Plusieurs racines XML."));
                    }
                    nodes.push(Node {
                        name,
                        attrs,
                        children: vec![],
                        text: String::new(),
                        start,
                        open_end: end,
                        close_start: end,
                        end,
                        empty,
                    });
                    if !empty {
                        stack.push(id);
                    }
                }
                Event::End(_) => {
                    let id = stack.pop().ok_or_else(|| error("XML mal forme."))?;
                    nodes[id].close_start = start;
                    nodes[id].end = end;
                }
                Event::Text(value) => {
                    let value = value
                        .unescape()
                        .map_err(|e| io_error("Entite XML invalide", e))?;
                    if !xml_text_valid(&value) {
                        return Err(error("Caractere XML interdit."));
                    }
                    if let Some(id) = stack.last() {
                        nodes[*id].text.push_str(&value);
                    } else if !value.trim().is_empty() {
                        return Err(error("Texte hors racine XML."));
                    }
                }
                Event::CData(value) => {
                    let value = std::str::from_utf8(value.as_ref())
                        .map_err(|e| io_error("XML invalide", e))?;
                    if !xml_text_valid(value) {
                        return Err(error("Caractere XML interdit."));
                    }
                    if let Some(id) = stack.last() {
                        nodes[*id].text.push_str(value);
                    } else {
                        return Err(error("CDATA hors racine XML."));
                    }
                }
                Event::DocType(_) => return Err(error("DTD XML interdite.")),
                Event::Decl(value) => {
                    if let Some(encoding) = value.encoding() {
                        let encoding =
                            encoding.map_err(|e| io_error("Encodage XML invalide", e))?;
                        if !encoding.eq_ignore_ascii_case(b"UTF-8") {
                            return Err(error("Seul le XML UTF-8 est pris en charge."));
                        }
                    }
                }
                Event::Eof => break,
                _ => {}
            }
        }
        if !stack.is_empty() {
            return Err(error("Document XML tronque."));
        }
        job.check()?;
        Ok(Self {
            nodes,
            root: root.ok_or_else(|| error("Document XML vide."))?,
        })
    }
    pub fn child(&self, id: usize, local: &str) -> Option<usize> {
        self.nodes[id]
            .children
            .iter()
            .copied()
            .find(|i| self.nodes[*i].local() == local)
    }
    pub fn children<'a>(&'a self, id: usize, local: &'a str) -> impl Iterator<Item = usize> + 'a {
        self.nodes[id]
            .children
            .iter()
            .copied()
            .filter(move |i| self.nodes[*i].local() == local)
    }
}

pub(crate) fn xml_text_valid(value: &str) -> bool {
    value.chars().all(|c| matches!(c as u32, 0x9 | 0xA | 0xD | 0x20..=0xD7FF | 0xE000..=0xFFFD | 0x10000..=0x10FFFF))
}
pub(crate) fn escape(value: &str) -> String {
    quick_xml::escape::escape(value).replace('\r', "&#13;")
}

pub(crate) type Patch = (usize, usize, Vec<u8>);
pub(crate) fn patch(bytes: &[u8], mut patches: Vec<Patch>) -> Result<Vec<u8>> {
    patches.sort_by_key(|(start, end, _)| (*start, *end));
    let mut output = Vec::new();
    let mut position = 0;
    for (start, end, content) in patches {
        if start < position || end < start || end > bytes.len() {
            return Err(error("Modifications XML incompatibles."));
        }
        output.extend_from_slice(&bytes[position..start]);
        output.extend(content);
        position = end;
    }
    output.extend_from_slice(&bytes[position..]);
    Ok(output)
}

pub(crate) fn append_children(
    bytes: &[u8],
    node: &Node,
    content: &str,
    updates: &[(&str, Option<String>)],
) -> Result<Vec<u8>> {
    if node.empty {
        patch(
            bytes,
            vec![(
                node.start,
                node.end,
                format!("{}{content}</{}>", node.tag(updates, false), node.name).into_bytes(),
            )],
        )
    } else {
        patch(
            bytes,
            vec![
                (
                    node.start,
                    node.open_end,
                    node.tag(updates, false).into_bytes(),
                ),
                (
                    node.close_start,
                    node.close_start,
                    content.as_bytes().to_vec(),
                ),
            ],
        )
    }
}

fn element_in_namespace(xml: &Xml, lineage: &[usize], namespace: &str) -> bool {
    let Some(&id) = lineage.last() else {
        return false;
    };
    let declaration = match xml.nodes[id].name.split_once(':') {
        Some((prefix, _)) => format!("xmlns:{prefix}"),
        None => "xmlns".into(),
    };
    lineage
        .iter()
        .rev()
        .find_map(|id| xml.nodes[*id].attr(&declaration))
        == Some(namespace)
}

fn inert_default_slicer_extension(xml: &Xml, id: usize) -> bool {
    // umya 3.1.1 always emits this empty default; no slicer definitions are allowed.
    let root = &xml.nodes[xml.root];
    let list = &xml.nodes[id];
    let only_attributes = |node: &Node, allowed: &[&str]| {
        node.attrs.iter().all(|(name, _)| {
            name == "xmlns" || name.starts_with("xmlns:") || allowed.contains(&name.as_str())
        })
    };
    if root.local() != "styleSheet"
        || !element_in_namespace(xml, &[xml.root], MAIN_NS)
        || !root.children.contains(&id)
        || xml.children(xml.root, "extLst").count() != 1
        || list.local() != "extLst"
        || !element_in_namespace(xml, &[xml.root, id], MAIN_NS)
        || !only_attributes(list, &[])
        || !list.text.trim().is_empty()
        || list.children.len() != 1
    {
        return false;
    }
    let ext_id = list.children[0];
    let ext = &xml.nodes[ext_id];
    if ext.local() != "ext"
        || !element_in_namespace(xml, &[xml.root, id, ext_id], MAIN_NS)
        || ext.attr("uri") != Some("{EB79DEF2-80B8-43e5-95BD-54CBDDF9020C}")
        || !only_attributes(ext, &["uri"])
        || !ext.text.trim().is_empty()
        || ext.children.len() != 1
    {
        return false;
    }
    let style_id = ext.children[0];
    let style = &xml.nodes[style_id];
    style.local() == "slicerStyles"
        && element_in_namespace(
            xml,
            &[xml.root, id, ext_id, style_id],
            "http://schemas.microsoft.com/office/spreadsheetml/2009/9/main",
        )
        && style.attr("defaultSlicerStyle") == Some("SlicerStyleLight1")
        && only_attributes(style, &["defaultSlicerStyle"])
        && style.children.is_empty()
        && style.text.trim().is_empty()
}

#[derive(Clone)]
pub(crate) struct Package {
    pub parts: BTreeMap<String, Vec<u8>>,
}
impl Package {
    pub fn from_bytes(bytes: &[u8], job: &JobControl) -> Result<Self> {
        job.check()?;
        if bytes.starts_with(&[0xd0, 0xcf, 0x11, 0xe0, 0xa1, 0xb1, 0x1a, 0xe1]) {
            return Err(error("Conteneur Excel binaire ou chiffre non pris en charge. Ouvrez une copie dans Excel ou LibreOffice avec le mot de passe si necessaire, retirez le chiffrement sur cette copie et enregistrez-la au format .xlsx avant de la recharger. Verifiez toute perte de macros ou d'objets ; conservez l'original."));
        }
        if bytes.len() as u64 > MAX_FILE {
            return Err(error("Classeur compresse trop volumineux (256 Mio)."));
        }
        let mut archive = ZipArchive::new(Cursor::new(bytes))
            .map_err(|e| io_error("Classeur invalide ou chiffre", e))?;
        if archive.len() > MAX_ENTRIES {
            return Err(error("Trop d'entrees ZIP dans le classeur."));
        }
        let mut parts = BTreeMap::new();
        let mut folded = BTreeSet::new();
        let mut total = 0u64;
        for i in 0..archive.len() {
            job.check()?;
            let mut entry = archive
                .by_index(i)
                .map_err(|e| io_error("Entree ZIP invalide ou chiffree", e))?;
            let name = entry.name().to_string();
            if name.starts_with('/')
                || name.contains(['\\', ':', '\0', '%'])
                || name.split('/').any(|v| v == ".." || v == ".")
            {
                return Err(error("Chemin ZIP non autorise."));
            }
            if !folded.insert(name.to_lowercase()) {
                return Err(error("Entrees ZIP dupliquees."));
            }
            if entry
                .unix_mode()
                .is_some_and(|mode| mode & 0o170000 == 0o120000)
            {
                return Err(error("Liens ZIP interdits."));
            }
            if entry.is_dir() {
                continue;
            }
            total = total
                .checked_add(entry.size())
                .ok_or_else(|| error("Taille ZIP invalide."))?;
            if entry.size() > MAX_PART
                || total > MAX_TOTAL
                || entry.size() > entry.compressed_size().max(1).saturating_mul(1000)
            {
                return Err(error(
                    "Limite ZIP depassee (partie 256 Mio, total 1 Go, ratio 1000).",
                ));
            }
            let lower = name.to_ascii_lowercase();
            let blocked = [
                "xl/drawings/",
                "xl/charts/",
                "xl/pivottables/",
                "xl/pivotcache/",
                "xl/slicers/",
                "xl/activex/",
                "xl/embeddings/",
                "xl/ctrlprops/",
                "_xmlsignatures/",
                "xl/externallinks/",
                "xl/querytables/",
                "xl/model/",
                "xl/richdata/",
                "xl/threadedcomments/",
                "xl/persons/",
                "customui/",
            ];
            if blocked.iter().any(|p| lower.starts_with(p))
                || lower.contains("vbaproject")
                || lower.contains("jsaproject")
                || matches!(lower.as_str(), "xl/connections.xml" | "xl/metadata.xml")
            {
                return Err(error(format!("Objet Excel non pris en charge : {name}.")));
            }
            let expected = entry.size();
            let mut content = Vec::new();
            let mut buffer = [0; 65_536];
            loop {
                job.check()?;
                let n = entry
                    .read(&mut buffer)
                    .map_err(|e| io_error("Entree ZIP endommagee", e))?;
                if n == 0 {
                    break;
                }
                if content.len() as u64 + n as u64 > expected {
                    return Err(error("Taille ZIP incoherente."));
                }
                content.extend_from_slice(&buffer[..n]);
            }
            if content.len() as u64 != expected {
                return Err(error("Entree ZIP tronquee."));
            }
            if lower.ends_with(".xml") || lower.ends_with(".rels") {
                let xml = Xml::parse(&content, job)?;
                for (id, node) in xml.nodes.iter().enumerate() {
                    if matches!(
                        node.local(),
                        "extLst"
                            | "AlternateContent"
                            | "oleObjects"
                            | "controls"
                            | "legacyDrawing"
                            | "drawing"
                            | "webPublishItems"
                    ) && !(name == "xl/styles.xml"
                        && node.local() == "extLst"
                        && inert_default_slicer_extension(&xml, id))
                    {
                        return Err(error(format!(
                            "Extensions ou objets Excel non conservables dans cette version : {name} ({}).",
                            node.name
                        )));
                    }
                    if node.local() == "Override" || node.local() == "Default" {
                        let kind = node.attr("ContentType").unwrap_or("").to_ascii_lowercase();
                        if [
                            "macroenabled",
                            "vbaproject",
                            "activex",
                            "oleobject",
                            "pivot",
                            "slicer",
                            "externallink",
                        ]
                        .iter()
                        .any(|v| kind.contains(v))
                        {
                            return Err(error("Type de contenu Excel non pris en charge (macros VBA, objets actifs ou donnees non conservables). Aucune conversion ou suppression implicite n'est autorisee."));
                        }
                    }
                }
            }
            parts.insert(name, content);
        }
        let package = Self { parts };
        for name in [
            "[Content_Types].xml",
            "_rels/.rels",
            "xl/workbook.xml",
            "xl/_rels/workbook.xml.rels",
        ] {
            package.get(name)?;
        }
        package.validate_relationships(job)?;
        package.format(job)?;
        Ok(package)
    }

    fn workbook_type_node(&self, job: &JobControl) -> Result<(Xml, usize)> {
        let xml = Xml::parse(self.get("[Content_Types].xml")?, job)?;
        let id = {
            let root = &xml.nodes[xml.root];
            let namespace = |node: &Node| {
                let key = node.name.rsplit_once(':').map_or_else(
                    || "xmlns".to_owned(),
                    |(prefix, _)| format!("xmlns:{prefix}"),
                );
                node.attr(&key).or_else(|| root.attr(&key)) == Some(CONTENT_NS)
            };
            if root.local() != "Types" || !namespace(root) {
                return Err(error("Manifeste de types de contenu Excel invalide."));
            }
            let mut matches = xml
                .children(xml.root, "Override")
                .filter(|id| xml.nodes[*id].attr("PartName") == Some("/xl/workbook.xml"));
            let id = matches
                .next()
                .ok_or_else(|| error("Type de contenu du classeur absent."))?;
            if matches.next().is_some() || !namespace(&xml.nodes[id]) {
                return Err(error("Type de contenu du classeur duplique ou invalide."));
            }
            id
        };
        Ok((xml, id))
    }

    pub fn format(&self, job: &JobControl) -> Result<WorkbookFormat> {
        let (xml, id) = self.workbook_type_node(job)?;
        match xml.nodes[id].required("ContentType")? {
            WORKBOOK_CONTENT => Ok(WorkbookFormat::Workbook),
            TEMPLATE_CONTENT => Ok(WorkbookFormat::Template),
            _ => Err(error("Type de classeur OOXML non pris en charge ; seuls .xlsx et .xltx sans macros sont conservables.")),
        }
    }

    pub fn normalize_workbook_type(&mut self, job: &JobControl) -> Result<()> {
        if self.format(job)? == WorkbookFormat::Workbook {
            return Ok(());
        }
        let (xml, id) = self.workbook_type_node(job)?;
        let node = &xml.nodes[id];
        let bytes = patch(
            self.get("[Content_Types].xml")?,
            vec![(
                node.start,
                node.open_end,
                node.tag(&[("ContentType", Some(WORKBOOK_CONTENT.into()))], node.empty)
                    .into_bytes(),
            )],
        )?;
        self.parts.insert("[Content_Types].xml".into(), bytes);
        Ok(())
    }
    pub fn get(&self, path: &str) -> Result<&[u8]> {
        self.parts
            .get(path)
            .map(Vec::as_slice)
            .ok_or_else(|| error(format!("Partie Excel manquante : {path}.")))
    }
    pub fn write<W: Write + Seek>(&self, writer: W, job: &JobControl) -> Result<W> {
        let mut zip = ZipWriter::new(writer);
        for (name, data) in &self.parts {
            job.check()?;
            zip.start_file(
                name,
                SimpleFileOptions::default().compression_method(CompressionMethod::Deflated),
            )
            .map_err(|e| io_error("Creation ZIP impossible", e))?;
            for chunk in data.chunks(65_536) {
                job.check()?;
                zip.write_all(chunk)?;
            }
        }
        zip.finish()
            .map_err(|e| io_error("Finalisation ZIP impossible", e))
    }
    fn validate_relationships(&self, job: &JobControl) -> Result<()> {
        let mut graph: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for (name, bytes) in self
            .parts
            .iter()
            .filter(|(name, _)| name.ends_with(".rels"))
        {
            let xml = Xml::parse(bytes, job)?;
            let mut ids = BTreeSet::new();
            let base = if name == "_rels/.rels" {
                String::new()
            } else {
                let (directory, file) = name
                    .rsplit_once("/_rels/")
                    .ok_or_else(|| error("Chemin de relations invalide."))?;
                format!("{directory}/{}", file.trim_end_matches(".rels"))
            };
            for id in xml.children(xml.root, "Relationship") {
                let node = &xml.nodes[id];
                if !ids.insert(node.required("Id")?) {
                    return Err(error("Identifiant de relation duplique."));
                }
                let kind = node.required("Type")?;
                if node.attr("TargetMode") == Some("External") {
                    if !kind.ends_with("/hyperlink") {
                        return Err(error("Relations externes actives non prises en charge."));
                    }
                    continue;
                }
                let target = relationship_target(&base, node.required("Target")?)?;
                self.get(&target)?;
                graph.entry(base.clone()).or_default().push(target);
            }
        }
        // umya recursively loads related parts. Refuse cycles before entering that
        // reader, including cycles expressed using otherwise unknown relation types.
        fn visit<'a>(
            node: &'a str,
            graph: &'a BTreeMap<String, Vec<String>>,
            states: &mut BTreeMap<&'a str, u8>,
            depth: usize,
            job: &JobControl,
        ) -> Result<()> {
            job.check()?;
            if depth > 64 || states.get(node) == Some(&1) {
                return Err(error("Relations Excel cycliques ou trop profondes."));
            }
            if states.get(node) == Some(&2) {
                return Ok(());
            }
            states.insert(node, 1);
            if let Some(children) = graph.get(node) {
                for child in children {
                    visit(child, graph, states, depth + 1, job)?;
                }
            }
            states.insert(node, 2);
            Ok(())
        }
        let mut states = BTreeMap::new();
        for node in graph.keys() {
            visit(node, &graph, &mut states, 0, job)?;
        }
        Ok(())
    }
}

pub(crate) fn relationship_target(base: &str, target: &str) -> Result<String> {
    if target.contains(['\\', ':', '%', '#', '?', '\0']) {
        return Err(error("Cible de relation Excel non prise en charge."));
    }
    let mut parts: Vec<&str> = if target.starts_with('/') {
        vec![]
    } else {
        base.rsplit_once('/')
            .map_or(vec![], |(p, _)| p.split('/').collect())
    };
    for part in target.trim_start_matches('/').split('/') {
        match part {
            "" | "." => {}
            ".." => {
                if parts.pop().is_none() {
                    return Err(error("Relation hors du classeur."));
                }
            }
            _ => parts.push(part),
        }
    }
    Ok(parts.join("/"))
}

#[derive(Clone)]
pub(crate) struct SheetPart {
    pub name: String,
    pub path: String,
}
pub(crate) fn sheet_parts(package: &Package, job: &JobControl) -> Result<Vec<SheetPart>> {
    let rel = Xml::parse(package.get("xl/_rels/workbook.xml.rels")?, job)?;
    let mut targets = BTreeMap::new();
    for id in rel.children(rel.root, "Relationship") {
        let node = &rel.nodes[id];
        targets.insert(
            node.required("Id")?.to_owned(),
            (
                node.required("Type")?.to_owned(),
                relationship_target("xl/workbook.xml", node.required("Target")?)?,
            ),
        );
    }
    let xml = Xml::parse(package.get("xl/workbook.xml")?, job)?;
    let root = &xml.nodes[xml.root];
    if root.local() != "workbook"
        || !root
            .attrs
            .iter()
            .any(|(k, v)| k.starts_with("xmlns") && v == MAIN_NS)
    {
        return Err(error(
            "Seuls les classeurs XLSX OOXML transitionnels sont pris en charge.",
        ));
    }
    let sheets = xml
        .child(xml.root, "sheets")
        .ok_or_else(|| error("Classeur sans feuilles."))?;
    let mut result = Vec::new();
    let mut names = BTreeSet::new();
    let mut paths = BTreeSet::new();
    for id in xml.children(sheets, "sheet") {
        let node = &xml.nodes[id];
        let name = node.required("name")?.to_owned();
        let rid = node
            .attrs
            .iter()
            .find(|(k, _)| k.ends_with(":id"))
            .map(|(_, v)| v)
            .ok_or_else(|| error("Relation de feuille absente."))?;
        let (kind, path) = targets
            .get(rid)
            .ok_or_else(|| error("Relation de feuille introuvable."))?;
        if kind != &format!("{REL_NS}/worksheet") {
            return Err(error("Feuille graphique ou macro non prise en charge."));
        }
        if name.is_empty()
            || name.chars().count() > 31
            || !names.insert(name.to_lowercase())
            || !paths.insert(path.clone())
        {
            return Err(error("Feuilles dupliquees ou nom invalide."));
        }
        package.get(path)?;
        result.push(SheetPart {
            name,
            path: path.clone(),
        });
    }
    if result.is_empty() {
        return Err(error("Classeur sans feuilles."));
    }
    Ok(result)
}

pub(crate) fn coordinate(value: &str) -> Result<(u32, u32)> {
    let value = value.trim_matches('$');
    let mut column = 0u32;
    let mut row = String::new();
    for c in value.chars() {
        if c == '$' {
            continue;
        }
        if c.is_ascii_uppercase() && row.is_empty() {
            column = column
                .checked_mul(26)
                .and_then(|v| v.checked_add(c as u32 - 'A' as u32 + 1))
                .ok_or_else(|| error("Colonne Excel invalide."))?;
        } else if c.is_ascii_digit() {
            row.push(c);
        } else {
            return Err(error("Coordonnee Excel invalide."));
        }
    }
    let row: u32 = row.parse().map_err(|_| error("Ligne Excel invalide."))?;
    if !(1..=1_048_576).contains(&row) || !(1..=16_384).contains(&column) {
        return Err(error("Coordonnee hors limites Excel."));
    }
    Ok((row, column))
}
pub(crate) fn address(row: u32, mut column: u32) -> String {
    let mut letters = Vec::new();
    while column != 0 {
        column -= 1;
        letters.push((b'A' + (column % 26) as u8) as char);
        column /= 26;
    }
    format!("{}{row}", letters.into_iter().rev().collect::<String>())
}

pub(crate) fn number(value: &str) -> Result<BigDecimal> {
    if value.is_empty() || value.len() > 1024 {
        return Err(error("Nombre Excel invalide ou trop long."));
    }
    let number = BigDecimal::from_str(value).map_err(|_| error("Nombre Excel invalide."))?;
    let (_, exponent) = number.as_bigint_and_exponent();
    if !(-10_000..=10_000).contains(&exponent) {
        return Err(error("Exposant numerique hors limites."));
    }
    Ok(number)
}
pub(crate) fn equivalent(left: &CellValue, right: &CellValue) -> bool {
    if left.is_empty() && right.is_empty() {
        return true;
    }
    if matches!(left.kind.as_str(), "integer" | "decimal")
        && matches!(right.kind.as_str(), "integer" | "decimal")
    {
        return match (number(&left.value), number(&right.value)) {
            (Ok(a), Ok(b)) => a == b,
            _ => false,
        };
    }
    left == right
}

pub(crate) fn builtin_format(id: u32) -> Option<&'static str> {
    Some(match id {
        0 => "General",
        1 => "0",
        2 => "0.00",
        3 => "#,##0",
        4 => "#,##0.00",
        5 => "\"$\"#,##0_);(\"$\"#,##0)",
        6 => "\"$\"#,##0_);[Red](\"$\"#,##0)",
        7 => "\"$\"#,##0.00_);(\"$\"#,##0.00)",
        8 => "\"$\"#,##0.00_);[Red](\"$\"#,##0.00)",
        9 => "0%",
        10 => "0.00%",
        11 => "0.00E+00",
        12 => "# ?/?",
        13 => "# ??/??",
        14 => "mm-dd-yy",
        15 => "d-mmm-yy",
        16 => "d-mmm",
        17 => "mmm-yy",
        18 => "h:mm AM/PM",
        19 => "h:mm:ss AM/PM",
        20 => "h:mm",
        21 => "h:mm:ss",
        22 => "m/d/yy h:mm",
        37 => "#,##0 ;(#,##0)",
        38 => "#,##0 ;[Red](#,##0)",
        39 => "#,##0.00;(#,##0.00)",
        40 => "#,##0.00;[Red](#,##0.00)",
        41 => "_(* #,##0_);_(* (#,##0);_(* \"-\"_);_(@_)",
        42 => "_(\"$\"* #,##0_);_(\"$\"* (#,##0);_(\"$\"* \"-\"_);_(@_)",
        43 => "_(* #,##0.00_);_(* (#,##0.00);_(* \"-\"??_);_(@_)",
        44 => "_(\"$\"* #,##0.00_);_(\"$\"* (#,##0.00);_(\"$\"* \"-\"??_);_(@_)",
        45 => "mm:ss",
        46 => "[h]:mm:ss",
        47 => "mmss.0",
        48 => "##0.0E+0",
        49 => "@",
        _ => return None,
    })
}

pub(crate) fn format_tokens(code: &str) -> Vec<String> {
    let mut chars = code.chars().peekable();
    let mut tokens = Vec::new();
    while let Some(c) = chars.next() {
        let mut token = c.to_string();
        if c == '"' || c == '[' {
            let end = if c == '[' { ']' } else { '"' };
            for next in chars.by_ref() {
                token.push(next);
                if next == end {
                    break;
                }
            }
        } else if matches!(c, '\\' | '_' | '*') {
            if let Some(next) = chars.next() {
                token.push(next);
            }
        }
        tokens.push(token);
    }
    tokens
}
pub(crate) fn date_format(code: &str) -> bool {
    format_tokens(code)
        .iter()
        .any(|t| matches!(t.to_ascii_lowercase().as_str(), "d" | "y"))
}

pub(crate) struct Styles {
    pub formats: Vec<String>,
}
impl Styles {
    pub fn read(package: &Package, job: &JobControl) -> Result<Self> {
        let Some(bytes) = package.parts.get("xl/styles.xml") else {
            return Ok(Self {
                formats: vec!["General".into()],
            });
        };
        let xml = Xml::parse(bytes, job)?;
        let mut custom = BTreeMap::new();
        if let Some(parent) = xml.child(xml.root, "numFmts") {
            for id in xml.children(parent, "numFmt") {
                let node = &xml.nodes[id];
                let number: u32 = node
                    .required("numFmtId")?
                    .parse()
                    .map_err(|_| error("Format numerique invalide."))?;
                if custom
                    .insert(number, node.required("formatCode")?.to_owned())
                    .is_some()
                {
                    return Err(error("Format numerique duplique."));
                }
            }
        }
        let mut formats = Vec::new();
        if let Some(parent) = xml.child(xml.root, "cellXfs") {
            for id in xml.children(parent, "xf") {
                let num: u32 = xml.nodes[id]
                    .attr("numFmtId")
                    .unwrap_or("0")
                    .parse()
                    .map_err(|_| error("Style Excel invalide."))?;
                let code = custom
                    .get(&num)
                    .cloned()
                    .or_else(|| builtin_format(num).map(str::to_owned));
                // Locale-dependent built-ins cannot be rewritten without knowing their
                // semantics. Refusal is safer than silently replacing a date/currency.
                formats.push(code.ok_or_else(|| {
                    error(format!("Format Excel localise non pris en charge : {num}."))
                })?);
            }
        }
        if formats.is_empty() {
            formats.push("General".into());
        }
        Ok(Self { formats })
    }
}

fn rich_text(xml: &Xml, id: usize) -> String {
    let mut text = String::new();
    for child in &xml.nodes[id].children {
        match xml.nodes[*child].local() {
            "t" => text.push_str(&xml.nodes[*child].text),
            "r" => text.push_str(&rich_text(xml, *child)),
            _ => {}
        }
    }
    decode_excel_text(&text)
}

pub(crate) fn decode_excel_text(value: &str) -> String {
    let mut output = String::new();
    let mut offset = 0;
    let bytes = value.as_bytes();
    while offset < bytes.len() {
        if offset + 7 <= bytes.len()
            && bytes[offset] == b'_'
            && bytes[offset + 1] == b'x'
            && bytes[offset + 6] == b'_'
        {
            if let Ok(hex) = std::str::from_utf8(&bytes[offset + 2..offset + 6]) {
                if let Ok(code) = u16::from_str_radix(hex, 16) {
                    if (0xD800..=0xDBFF).contains(&code)
                        && offset + 14 <= bytes.len()
                        && &bytes[offset + 7..offset + 9] == b"_x"
                        && bytes[offset + 13] == b'_'
                    {
                        if let Ok(low) = std::str::from_utf8(&bytes[offset + 9..offset + 13]) {
                            if let Ok(low) = u16::from_str_radix(low, 16) {
                                if (0xDC00..=0xDFFF).contains(&low) {
                                    let point = 0x10000
                                        + ((u32::from(code) - 0xD800) << 10)
                                        + u32::from(low)
                                        - 0xDC00;
                                    if let Some(c) = char::from_u32(point) {
                                        output.push(c);
                                        offset += 14;
                                        continue;
                                    }
                                }
                            }
                        }
                    }
                    if let Some(c) = char::from_u32(code as u32) {
                        output.push(c);
                        offset += 7;
                        continue;
                    }
                }
            }
        }
        if let Some(c) = value[offset..].chars().next() {
            output.push(c);
            offset += c.len_utf8();
        }
    }
    output
}
pub(crate) fn encode_excel_text(value: &str) -> String {
    let mut output = String::new();
    let bytes = value.as_bytes();
    for (offset, c) in value.char_indices() {
        if c == '_'
            && bytes.get(offset + 1) == Some(&b'x')
            && bytes.get(offset + 6) == Some(&b'_')
            && bytes
                .get(offset + 2..offset + 6)
                .is_some_and(|v| v.iter().all(u8::is_ascii_hexdigit))
        {
            output.push_str("_x005F_");
        } else if !xml_text_valid(&c.to_string()) {
            output.push_str(&format!("_x{:04X}_", c as u32));
        } else {
            output.push(c);
        }
    }
    output
}

pub(crate) fn serial_date(raw: &str, date1904: bool) -> Option<String> {
    let number = number(raw).ok()?;
    let serial = number.to_i64()?;
    if number != serial || !(0..=2_958_465).contains(&serial) || (!date1904 && serial == 60) {
        return None;
    }
    let base = if date1904 {
        NaiveDate::from_ymd_opt(1904, 1, 1)?
    } else {
        NaiveDate::from_ymd_opt(1899, 12, 31)?
    };
    let days = if !date1904 && serial > 60 {
        serial - 1
    } else {
        serial
    };
    let date = base.checked_add_signed(Duration::days(days))?;
    if !(1900..=9999).contains(&date.year()) && !date1904 {
        return None;
    }
    if date.year() > 9999 {
        return None;
    }
    Some(date.format("%Y-%m-%d").to_string())
}

pub(crate) fn data_from_package(
    package: &Package,
    source_hash: String,
    job: &JobControl,
) -> Result<WorkbookData> {
    let parts = sheet_parts(package, job)?;
    let styles = Styles::read(package, job)?;
    let workbook = Xml::parse(package.get("xl/workbook.xml")?, job)?;
    let date1904 = workbook
        .child(workbook.root, "workbookPr")
        .is_some_and(|id| matches!(workbook.nodes[id].attr("date1904"), Some("1" | "true")));
    let mut strings = Vec::new();
    if let Some(bytes) = package.parts.get("xl/sharedStrings.xml") {
        let xml = Xml::parse(bytes, job)?;
        for id in xml.children(xml.root, "si") {
            strings.push(rich_text(&xml, id));
        }
    }
    let mut sheets = Vec::new();
    for (index, part) in parts.iter().enumerate() {
        job.check()?;
        job.report(
            "lecture",
            index as u64,
            Some(parts.len() as u64),
            "Lecture exacte des cellules",
        );
        let xml = Xml::parse(package.get(&part.path)?, job)?;
        if xml.nodes[xml.root].local() != "worksheet" {
            return Err(error("Feuille Excel invalide."));
        }
        let mut sheet = SheetData {
            name: part.name.clone(),
            rows: 1,
            columns: 1,
            cells: BTreeMap::new(),
        };
        if let Some(id) = xml.child(xml.root, "dimension") {
            let range = xml.nodes[id].required("ref")?;
            let (row, col) = coordinate(range.rsplit(':').next().unwrap_or(range))?;
            sheet.rows = row;
            sheet.columns = col;
        }
        let sheet_data = xml
            .child(xml.root, "sheetData")
            .ok_or_else(|| error("Donnees de feuille absentes."))?;
        let mut previous_row = 0;
        for row_id in xml.children(sheet_data, "row") {
            job.check()?;
            let row: u32 = xml.nodes[row_id]
                .required("r")?
                .parse()
                .map_err(|_| error("Ligne Excel invalide."))?;
            if row <= previous_row || row > 1_048_576 {
                return Err(error("Lignes Excel dupliquees ou non ordonnees."));
            }
            previous_row = row;
            sheet.rows = sheet.rows.max(row);
            let mut previous_col = 0;
            for cell_id in xml.children(row_id, "c") {
                let cell = &xml.nodes[cell_id];
                let (cell_row, col) = coordinate(cell.required("r")?)?;
                if cell_row != row || col <= previous_col {
                    return Err(error("Cellules Excel dupliquees ou non ordonnees."));
                }
                previous_col = col;
                sheet.columns = sheet.columns.max(col);
                let raw = xml
                    .child(cell_id, "v")
                    .map(|id| xml.nodes[id].text.as_str())
                    .unwrap_or("");
                let inherited_column = xml.child(xml.root, "cols").and_then(|cols| {
                    xml.children(cols, "col").find_map(|id| {
                        let node = &xml.nodes[id];
                        let min = node.attr("min")?.parse::<u32>().ok()?;
                        let max = node.attr("max")?.parse::<u32>().ok()?;
                        if (min..=max).contains(&col) {
                            node.attr("style")
                        } else {
                            None
                        }
                    })
                });
                let style: usize = cell
                    .attr("s")
                    .or_else(|| xml.nodes[row_id].attr("s"))
                    .or(inherited_column)
                    .unwrap_or("0")
                    .parse()
                    .map_err(|_| error("Style Excel invalide."))?;
                let format = styles
                    .formats
                    .get(style)
                    .ok_or_else(|| error("Style Excel introuvable."))?;
                let value = if let Some(id) = xml.child(cell_id, "f") {
                    CellValue::typed("formula", format!("={}", xml.nodes[id].text))
                } else {
                    match cell.attr("t").unwrap_or("n") {
                        "s" => {
                            let index: usize = raw
                                .parse()
                                .map_err(|_| error("Index de texte Excel invalide."))?;
                            CellValue::text(
                                strings
                                    .get(index)
                                    .ok_or_else(|| error("Texte Excel introuvable."))?
                                    .clone(),
                            )
                        }
                        "inlineStr" => CellValue::text(
                            xml.child(cell_id, "is")
                                .map(|id| rich_text(&xml, id))
                                .unwrap_or_default(),
                        ),
                        "str" => CellValue::text(decode_excel_text(raw)),
                        "b" => CellValue::typed(
                            "boolean",
                            match raw {
                                "1" | "true" => "true",
                                "0" | "false" => "false",
                                _ => return Err(error("Booleen Excel invalide.")),
                            },
                        ),
                        "e" => CellValue::typed("error", raw),
                        "d" => {
                            if NaiveDate::parse_from_str(raw, "%Y-%m-%d").is_err() {
                                return Err(error("Date ISO avec heure non prise en charge ; aucune troncature autorisee."));
                            }
                            CellValue::typed("date", raw)
                        }
                        "n" if raw.is_empty() => CellValue::empty(),
                        "n" => {
                            number(raw)?;
                            if date_format(format) {
                                serial_date(raw, date1904)
                                    .map(|v| CellValue::typed("date", v))
                                    .unwrap_or_else(|| CellValue::typed("decimal", raw))
                            } else {
                                CellValue::typed("decimal", raw)
                            }
                        }
                        _ => return Err(error("Type de cellule Excel non pris en charge.")),
                    }
                };
                sheet.cells.insert((row, col), value);
                if sheet.cells.len() > 5_000_000 {
                    return Err(error(
                        "Limite de cinq millions de cellules par feuille depassee.",
                    ));
                }
            }
        }
        sheets.push(sheet);
    }
    Ok(WorkbookData {
        source_hash,
        sheets,
    })
}

pub(crate) fn umya_read(bytes: &[u8], job: &JobControl) -> Result<umya_spreadsheet::Workbook> {
    job.check()?;
    // Lazy worksheets retain their original XML. Do not obtain numeric values through
    // umya's f64 API; the guarded XML reader above is the source of typed values.
    let result = std::panic::catch_unwind(|| {
        umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(bytes), false)
    })
    .map_err(|_| error("Classeur refuse par le lecteur Excel ; aucune modification effectuee."))?
    .map_err(|e| io_error("Lecture Excel impossible", e))?;
    job.check()?;
    Ok(result)
}

pub fn read_source(path: &Path, job: &JobControl) -> Result<WorkbookData> {
    let (bytes, package) = read_source_package(path, job)?;
    read_source_data(path, &bytes, &package, job).map_err(|e| source_error(path, e))
}

fn read_source_data(
    path: &Path,
    bytes: &[u8],
    package: &Package,
    job: &JobControl,
) -> Result<WorkbookData> {
    let hash = digest(bytes);
    let data = data_from_package(package, hash.clone(), job)?;
    let book = umya_read(bytes, job)?;
    if book
        .sheet_collection_no_check()
        .iter()
        .map(|s| s.name())
        .ne(data.sheets.iter().map(|s| s.name.as_str()))
    {
        return Err(error(
            "Structure des feuilles incoherente entre les lecteurs Excel.",
        ));
    }
    if hash_file(path, job)? != hash {
        return Err(error("La source a change pendant la lecture."));
    }
    Ok(data)
}

pub fn inspect(path: &Path, header_row: u32, job: &JobControl) -> Result<WorkbookInfo> {
    if !(1..=1_048_576).contains(&header_row) {
        return Err(error("Ligne d'en-tete hors limites Excel."));
    }
    let data = read_source(path, job)?;
    let mut sheets = Vec::new();
    for sheet in data.sheets {
        job.check()?;
        let mut suggested_header = 1;
        let mut best = i64::MIN;
        for row in 1..=sheet.rows.min(20) {
            let filled: Vec<_> = sheet
                .cells
                .range((row, 0)..=(row, u32::MAX))
                .map(|(_, v)| v)
                .filter(|v| !v.is_empty())
                .collect();
            let text = filled.iter().filter(|v| v.kind == "text").count() as i64;
            let unique = filled
                .iter()
                .map(|v| v.value.trim().to_lowercase())
                .collect::<BTreeSet<_>>()
                .len() as i64;
            let score = if filled.len() <= 1 {
                0
            } else {
                text * 2 + unique - (filled.len() as i64 - text) * 2
            };
            if score > best {
                best = score;
                suggested_header = row;
            }
        }
        let headers = if header_row <= sheet.rows {
            (1..=sheet.columns)
                .map(|col| sheet.cell(header_row, col))
                .collect()
        } else {
            vec![]
        };
        let preview = (header_row + 1..=sheet.rows.min(header_row + 8))
            .map(|row| {
                (1..=sheet.columns)
                    .map(|col| sheet.cell(row, col))
                    .collect()
            })
            .collect();
        sheets.push(SheetInfo {
            name: sheet.name,
            rows: sheet.rows,
            columns: sheet.columns,
            headers,
            preview,
            suggested_header,
        });
    }
    let is_report = |name: &str| {
        matches!(
            name.to_lowercase().as_str(),
            "contr\u{f4}le" | "nom_original" | "\u{e0} v\u{e9}rifier" | "a verifier"
        )
    };
    let suggested_sheet = sheets
        .iter()
        .filter(|s| !is_report(&s.name))
        .max_by_key(|s| s.rows as u64 * s.columns as u64)
        .or_else(|| sheets.first())
        .map(|s| s.name.clone())
        .unwrap_or_default();
    job.check()?;
    Ok(WorkbookInfo {
        path: path.to_string_lossy().into_owned(),
        sheets,
        suggested_sheet,
    })
}
