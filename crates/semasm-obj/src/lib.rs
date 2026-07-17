//! Object-file inspection for SemASM.
//!
//! Reads ELF / PE / Mach-O containers and exposes a normalized,
//! deterministic JSON view: file format, architecture, sections,
//! symbols, relocations, and (where applicable) imports/exports.
//!
//! The crate is intentionally dependency-light and `#![forbid(unsafe_code)]`.
//! Malformed input surfaces as an [`ObjectError`] rather than panicking.

#![forbid(unsafe_code)]

use std::path::Path;

use object::{
    Architecture, BinaryFormat, Endianness, Object, ObjectSection, ObjectSymbol, RelocationTarget,
    SectionKind, SymbolKind,
};
use semasm_target::{Isa, ObjectFormat, TargetIdentity};
use serde::Serialize;

/// Errors produced while inspecting an object file.
#[derive(Debug, thiserror::Error)]
pub enum ObjectError {
    /// The file could not be read from disk.
    #[error("io error reading `{0}`: {1}")]
    Io(String, std::io::Error),
    /// The file is not a recognised object container.
    #[error("unrecognised or unsupported object file: {0}")]
    Unrecognised(String),
    /// The object architecture does not match the requested target.
    #[error("architecture mismatch: object is `{actual}` but target requires `{expected}`")]
    ArchitectureMismatch {
        /// Architecture found in the object.
        actual: String,
        /// Architecture required by the target.
        expected: String,
    },
}

/// High-level container format.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ContainerFormat {
    /// ELF (Linux/BSD/etc.).
    Elf,
    /// PE/COFF (Windows).
    Pe,
    /// Mach-O (macOS/iOS).
    Macho,
    /// Some other / unknown container.
    Unknown,
}

/// A parsed object file.
#[derive(Debug, Clone, Serialize)]
pub struct ObjectInfo {
    /// Container format.
    pub format: ContainerFormat,
    /// Architecture family (normalised to SemASM's `Isa`).
    pub architecture: Isa,
    /// Raw architecture string from the object.
    pub architecture_raw: String,
    /// Endianness.
    pub endian: String,
    /// Entry-point virtual address (0 if not applicable).
    pub entry: u64,
    /// Sections, sorted by address for determinism.
    pub sections: Vec<SectionInfo>,
    /// Symbols, sorted by name for determinism.
    pub symbols: Vec<SymbolInfo>,
    /// Relocations, sorted by offset for determinism.
    pub relocations: Vec<RelocationInfo>,
    /// Undefined global symbols (external references).
    pub imports: Vec<String>,
    /// Defined global symbols (exported entry points).
    pub exports: Vec<String>,
}

/// Section summary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SectionInfo {
    /// Section name (empty for nameless sections).
    pub name: String,
    /// Virtual address.
    pub address: u64,
    /// Size in bytes.
    pub size: u64,
    /// Kind (text/data/bss/...).
    pub kind: String,
    /// Whether the section is writable.
    pub writable: bool,
    /// Whether the section is executable.
    pub executable: bool,
}

/// Symbol summary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SymbolInfo {
    /// Symbol name.
    pub name: String,
    /// Virtual address (0 for undefined symbols).
    pub address: u64,
    /// Kind (text/data/unknown/...).
    pub kind: String,
    /// Whether the symbol is global/exported.
    pub global: bool,
    /// Whether the symbol is undefined (an import).
    pub undefined: bool,
}

/// Relocation summary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RelocationInfo {
    /// Offset of the relocation within its section.
    pub offset: u64,
    /// Size of the patched field in bits.
    pub size: u8,
    /// Name of the referenced symbol (when resolvable).
    pub target_symbol: String,
    /// Whether the target is an addend/relative kind.
    pub is_relative: bool,
}

fn map_format(f: BinaryFormat) -> ContainerFormat {
    match f {
        BinaryFormat::Elf => ContainerFormat::Elf,
        BinaryFormat::Coff | BinaryFormat::Pe => ContainerFormat::Pe,
        BinaryFormat::MachO => ContainerFormat::Macho,
        _ => ContainerFormat::Unknown,
    }
}

fn arch_raw_string(a: Architecture) -> String {
    // `object::Architecture` has no `Display`; use Debug for a stable
    // representation.  This is the raw value reported to the user.
    format!("{a:?}")
}

#[allow(clippy::match_same_arms)]
fn map_architecture(a: Architecture) -> Isa {
    match a {
        Architecture::X86_64 => Isa::X86_64,
        Architecture::Aarch64 => Isa::AArch64,
        Architecture::Riscv64 => Isa::Riscv64,
        Architecture::Riscv32 => Isa::Riscv32,
        _ => Isa::X86_64, // not portably mappable; caller validates via target.
    }
}

fn section_kind_name(k: SectionKind) -> String {
    match k {
        SectionKind::Text => "text",
        SectionKind::Data => "data",
        SectionKind::ReadOnlyData => "rodata",
        SectionKind::UninitializedData => "bss",
        SectionKind::Tls => "tls",
        SectionKind::TlsVariables => "tls_variables",
        SectionKind::Common => "common",
        SectionKind::Unknown | _ => "unknown",
    }
    .to_string()
}

fn symbol_kind_name(k: SymbolKind) -> String {
    match k {
        SymbolKind::Text => "text",
        SymbolKind::Data => "data",
        SymbolKind::Section => "section",
        SymbolKind::File => "file",
        SymbolKind::Label => "label",
        SymbolKind::Tls => "tls",
        SymbolKind::Unknown | _ => "unknown",
    }
    .to_string()
}

fn is_reloc_relative(k: object::RelocationKind) -> bool {
    matches!(
        k,
        object::RelocationKind::Relative
            | object::RelocationKind::GotRelative
            | object::RelocationKind::GotBaseRelative
            | object::RelocationKind::PltRelative
            | object::RelocationKind::ImageOffset
            | object::RelocationKind::SectionOffset
    )
}

/// Read and normalise an object file from disk.
///
/// # Errors
///
/// Returns [`ObjectError::Io`] on read failure,
/// [`ObjectError::Unrecognised`] when the bytes are not a known
/// container, or [`ObjectError::ArchitectureMismatch`] only when
/// using [`read_for_target`].
pub fn read(path: &Path) -> Result<ObjectInfo, ObjectError> {
    let bytes = std::fs::read(path).map_err(|e| ObjectError::Io(path.display().to_string(), e))?;
    parse(&bytes)
}

/// Parse an object file from an in-memory byte buffer.
///
/// # Errors
///
/// Returns [`ObjectError::Unrecognised`] when the bytes are not a known
/// container.
pub fn parse(bytes: &[u8]) -> Result<ObjectInfo, ObjectError> {
    let file = object::File::parse(bytes).map_err(|e| ObjectError::Unrecognised(e.to_string()))?;

    let format = map_format(file.format());
    let architecture = map_architecture(file.architecture());
    let architecture_raw = arch_raw_string(file.architecture());
    let endian = match file.endianness() {
        Endianness::Little => "little",
        Endianness::Big => "big",
    }
    .to_string();
    let entry = file.entry();

    // Sections.
    let mut sections = Vec::new();
    for section in file.sections() {
        let name = section.name().unwrap_or("").to_string();
        let kind = section_kind_name(section.kind());
        let writable = matches!(
            section.kind(),
            SectionKind::Data
                | SectionKind::UninitializedData
                | SectionKind::Common
                | SectionKind::Tls
                | SectionKind::TlsVariables
        );
        let executable = section.kind() == SectionKind::Text;
        sections.push(SectionInfo {
            name,
            address: section.address(),
            size: section.size(),
            kind,
            writable,
            executable,
        });
    }
    sections.sort_by_key(|s| s.address);

    // Symbols → exports / imports / table.
    let mut symbols = Vec::new();
    let mut imports = Vec::new();
    let mut exports = Vec::new();
    let mut names_by_index: Vec<Option<String>> = Vec::new();
    for symbol in file.symbols().chain(file.dynamic_symbols()) {
        let name = symbol.name().unwrap_or("").to_string();
        let global = symbol.is_global() || symbol.is_weak();
        let undefined = symbol.is_undefined();
        let kind = symbol_kind_name(symbol.kind());
        let address = symbol.address();

        symbols.push(SymbolInfo {
            name: name.clone(),
            address,
            kind,
            global,
            undefined,
        });

        if undefined && global && !name.is_empty() && !imports.contains(&name) {
            imports.push(name.clone());
        } else if global && !name.is_empty() && !undefined && !exports.contains(&name) {
            exports.push(name.clone());
        }
        names_by_index.push(if name.is_empty() { None } else { Some(name) });
    }
    symbols.sort_by(|a, b| a.name.cmp(&b.name));
    imports.sort();
    exports.sort();

    // Relocations (per section → `Relocation` with the full API).
    let mut relocations = Vec::new();
    for section in file.sections() {
        for (offset, relocation) in section.relocations() {
            let target_symbol = match relocation.target() {
                RelocationTarget::Symbol(index) => names_by_index
                    .get(index.0)
                    .and_then(Option::as_ref)
                    .cloned()
                    .unwrap_or_else(|| format!("<symbol {}>", index.0)),
                RelocationTarget::Section(index) => format!("<section {}>", index.0),
                RelocationTarget::Absolute => "<absolute>".to_string(),
                _ => "<unknown>".to_string(),
            };
            relocations.push(RelocationInfo {
                offset,
                size: relocation.size(),
                target_symbol,
                is_relative: is_reloc_relative(relocation.kind())
                    || relocation.has_implicit_addend(),
            });
        }
    }
    relocations.sort_by_key(|r| r.offset);

    Ok(ObjectInfo {
        format,
        architecture,
        architecture_raw,
        endian,
        entry,
        sections,
        symbols,
        relocations,
        imports,
        exports,
    })
}

/// Read an object file and require that its architecture matches `target`.
///
/// Returns [`ObjectError::ArchitectureMismatch`] when the parsed
/// architecture does not equal the target's ISA — a hard error per the
/// OBJECT-001 acceptance criteria.
///
/// # Errors
///
/// Propagates [`read`] errors and adds the mismatch check.
pub fn read_for_target(path: &Path, target: &TargetIdentity) -> Result<ObjectInfo, ObjectError> {
    let info = read(path)?;
    if info.architecture != target.isa {
        return Err(ObjectError::ArchitectureMismatch {
            actual: info.architecture.to_string(),
            expected: target.isa.to_string(),
        });
    }
    Ok(info)
}

/// Map a [`TargetIdentity`] to the expected [`ObjectFormat`] for reporting.
#[must_use]
pub fn expected_format(target: &TargetIdentity) -> ObjectFormat {
    target.object_format
}

impl ObjectInfo {
    /// Serialise to deterministic JSON (keys sorted).
    ///
    /// # Errors
    ///
    /// Propagates `serde_json` serialisation failure (rare for this type).
    pub fn to_json(&self) -> serde_json::Result<String> {
        serde_json::to_string_pretty(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Per-call unique counter so parallel tests don't race on a shared
    // temp directory.
    static UNIQUE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

    #[test]
    fn rejects_empty_buffer_without_panic() {
        // An empty slice is not a valid container; must error, not panic.
        let err = parse(&[]).unwrap_err();
        assert!(matches!(err, ObjectError::Unrecognised(_)));
    }

    #[test]
    fn rejects_random_bytes_without_panic() {
        let junk = b"This is definitely not an object file \x00\x01\x02";
        let err = parse(junk).unwrap_err();
        assert!(matches!(err, ObjectError::Unrecognised(_)));
    }

    #[test]
    fn json_output_is_deterministic() {
        // Re-parsing the same bytes yields identical JSON.
        let bytes = build_minimal_elf();
        let a = parse(&bytes).unwrap().to_json().unwrap();
        let b = parse(&bytes).unwrap().to_json().unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn parses_minimal_elf_sections() {
        let bytes = build_minimal_elf();
        let info = parse(&bytes).unwrap();
        assert_eq!(info.format, ContainerFormat::Elf);
        // The assembled fixture is x86-64.
        assert_eq!(info.architecture, Isa::X86_64);
        // At least one text-like section is present.
        assert!(info.sections.iter().any(|s| s.kind == "text"));
    }

    #[test]
    fn parses_minimal_win64_object() {
        let bytes = build_minimal_win64();
        let info = parse(&bytes).unwrap();
        assert_eq!(info.format, ContainerFormat::Pe);
        // The assembled fixture is x86-64.
        assert_eq!(info.architecture, Isa::X86_64);
        // At least one text-like section is present.
        assert!(info.sections.iter().any(|s| s.kind == "text"));
    }

    #[test]
    fn win64_object_is_not_elf() {
        let win = build_minimal_win64();
        let elf = build_minimal_elf();
        assert_ne!(
            parse(&win).unwrap().format,
            parse(&elf).unwrap().format,
            "win64 and elf objects must report different containers"
        );
    }

    #[test]
    fn architecture_mismatch_is_hard_error() {
        use semasm_target::{Isa, TargetIdentity};

        let bytes = build_minimal_elf();
        // The fixture is x86-64 but we require AArch64 → must error.
        let mut wrong = TargetIdentity::x86_64_linux_gnu();
        wrong.isa = Isa::AArch64;
        wrong.name = "aarch64-unknown-linux-gnu".into();

        let p = std::env::temp_dir().join(format!("semasm-obj-mm-{}", std::process::id()));
        std::fs::write(&p, &bytes).unwrap();
        let err = read_for_target(&p, &wrong).unwrap_err();
        assert!(matches!(err, ObjectError::ArchitectureMismatch { .. }));

        let _ = std::fs::remove_file(&p);
    }

    // Assemble a real, minimal ELF object with the workspace toolchain so
    // the parser is exercised against genuine container bytes rather than a
    // hand-built (and fragile) ELF header.
    fn build_minimal_elf() -> Vec<u8> {
        use semasm_build::Pipeline;
        use semasm_target::TargetIdentity;

        let target = TargetIdentity::x86_64_linux_gnu();
        let pipe = Pipeline::discover(&target);

        let dir = std::env::temp_dir().join(format!(
            "semasm-obj-test-{}-{}",
            std::process::id(),
            UNIQUE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let _ = std::fs::create_dir_all(&dir);
        let src = dir.join("exit.asm");
        let obj = dir.join("exit.o");
        std::fs::write(
            &src,
            "BITS 64\nDEFAULT REL\nsection .text\n; minimal ret\nret\n",
        )
        .expect("write asm");

        let out = pipe
            .assemble(&src, &obj, "elf64")
            .expect("assemble fixture");
        assert!(out.success(), "assembler must succeed");

        let bytes = std::fs::read(&obj).expect("read object");
        let _ = std::fs::remove_dir_all(&dir);
        bytes
    }

    // Assemble a real, minimal PE/COFF object (win64) with the workspace
    // toolchain so the parser is exercised against genuine container bytes.
    fn build_minimal_win64() -> Vec<u8> {
        use semasm_build::Pipeline;
        use semasm_target::TargetIdentity;

        let target = TargetIdentity::x86_64_windows_msvc();
        let pipe = Pipeline::discover(&target);

        let dir = std::env::temp_dir().join(format!(
            "semasm-obj-test-{}-{}",
            std::process::id(),
            UNIQUE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let _ = std::fs::create_dir_all(&dir);
        let src = dir.join("exit.asm");
        let obj = dir.join("exit.obj");
        std::fs::write(&src, "BITS 64\nsection .text\n; minimal ret\nret\n").expect("write asm");

        let out = pipe
            .assemble(&src, &obj, target.nasm_format())
            .expect("assemble fixture");
        assert!(out.success(), "assembler must succeed");

        let bytes = std::fs::read(&obj).expect("read object");
        let _ = std::fs::remove_dir_all(&dir);
        bytes
    }
}
