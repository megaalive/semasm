//! Target authoring profiles for agent workspaces.
//!
//! Profiles describe assembler dialect, ABI register/stack facts, and known
//! incomplete patterns. They are authoring guidance — not acceptance authority.
//! RISC-V profiles may be generated, but agent-verify remains fail-closed at
//! the VAA controller until a dedicated gate exists.

use semasm_core::{Error, Result};
use serde::{Deserialize, Serialize};

use crate::abi::ABIRegisterMap;
use crate::{Dialect, Isa, ObjectFormat, TargetIdentity};

/// Assembler / ABI authoring profile for one registered target.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthoringProfile {
    /// Stable profile id (`{target}@{dialect}`).
    pub profile_id: String,
    /// Canonical target identity name.
    pub target: String,
    /// Human dialect label (`nasm-intel`, `gas-unified`, …).
    pub syntax: String,
    /// Same as [`Self::syntax`]; retained for consumers that key on `dialect`.
    pub dialect: String,
    /// Minimal source file skeleton agents may start from.
    pub file_template: String,
    /// Symbol / section naming rules for this dialect.
    pub symbol_rules: SymbolRules,
    /// ABI register map plus stack frame constants.
    pub abi: AuthoringAbi,
    /// Addressing modes the verifier models for this target.
    pub modeled_addressing: Vec<String>,
    /// Loop idioms covered by collectors (guidance only).
    pub supported_loop_idioms: Vec<String>,
    /// Patterns known incomplete / fail-closed for this target.
    pub known_incomplete_patterns: Vec<String>,
    /// Object container format (`elf`, `pe-coff`).
    pub object_format: String,
}

/// Symbol and section conventions for the assembler dialect.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SymbolRules {
    /// How to export a routine symbol (e.g. `global name`).
    pub export_directive: String,
    /// Text section directive.
    pub text_section: String,
    /// Whether the routine symbol needs a leading underscore.
    pub leading_underscore: bool,
    /// Preferred source file extension (`.asm`, `.S`).
    pub source_extension: String,
}

/// ABI facts agents need beyond the register map.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthoringAbi {
    /// Parameter / return / preserved / volatile registers.
    #[serde(flatten)]
    pub registers: ABIRegisterMap,
    /// Required stack alignment in bytes at call sites.
    pub stack_alignment: u32,
    /// Home / shadow space reserved by the caller (Win64 = 32).
    pub shadow_space_bytes: u32,
    /// Leaf red-zone bytes below RSP (SysV = 128; Win64 = 0).
    pub red_zone_bytes: u32,
}

impl AuthoringProfile {
    /// Build the authoring profile for a known target identity.
    pub fn for_target(target_id: &str) -> Result<Self> {
        let identity = TargetIdentity::parse_known(target_id)?;
        let registers = identity.abi_register_map().ok_or_else(|| {
            Error::not_found(format!(
                "no ABI register map registered for target `{}`",
                identity.name
            ))
        })?;
        let (stack_alignment, shadow_space_bytes, red_zone_bytes) = match identity.abi {
            crate::Abi::WindowsX64 => (16, 32, 0),
            crate::Abi::SysVAmd64 => (16, 0, 128),
            crate::Abi::Aapcs64 | crate::Abi::Riscv => (16, 0, 0),
        };
        let dialect = identity.dialect.to_string();
        let object_format = identity.object_format.to_string();
        let symbol_rules = symbol_rules_for(&identity);
        let file_template = file_template_for(&identity, &symbol_rules);
        let modeled_addressing = modeled_addressing_for(identity.isa);
        let known_incomplete_patterns = known_incomplete_patterns_for(&identity);
        Ok(Self {
            profile_id: format!("{}@{dialect}", identity.name),
            target: identity.name.clone(),
            syntax: dialect.clone(),
            dialect,
            file_template,
            symbol_rules,
            abi: AuthoringAbi {
                registers,
                stack_alignment,
                shadow_space_bytes,
                red_zone_bytes,
            },
            modeled_addressing,
            supported_loop_idioms: supported_loop_idioms(),
            known_incomplete_patterns,
            object_format,
        })
    }
}

fn symbol_rules_for(identity: &TargetIdentity) -> SymbolRules {
    match identity.dialect {
        Dialect::NasmIntel => SymbolRules {
            export_directive: "global".to_string(),
            text_section: "section .text".to_string(),
            leading_underscore: false,
            source_extension: ".asm".to_string(),
        },
        Dialect::GasAtt | Dialect::GasUnified => SymbolRules {
            export_directive: ".global".to_string(),
            text_section: ".text".to_string(),
            leading_underscore: false,
            source_extension: ".S".to_string(),
        },
    }
}

fn file_template_for(identity: &TargetIdentity, rules: &SymbolRules) -> String {
    match identity.dialect {
        Dialect::NasmIntel => format!(
            "; {{routine}} — agent candidate skeleton\n\
BITS 64\n\
DEFAULT REL\n\
\n\
{} {{routine}}\n\
\n\
{}\n\
{{routine}}:\n\
\t; TODO: implement\n\
\tret\n",
            rules.export_directive, rules.text_section
        ),
        Dialect::GasAtt | Dialect::GasUnified => match identity.isa {
            Isa::AArch64 => format!(
                "// {{routine}} — agent candidate skeleton (AArch64)\n\
{} {{routine}}\n\
{}\n\
{{routine}}:\n\
\t// TODO: implement\n\
\tret\n",
                rules.export_directive, rules.text_section
            ),
            Isa::Riscv64 | Isa::Riscv32 => format!(
                "// {{routine}} — agent candidate skeleton (RISC-V)\n\
// Note: VAA agent-verify remains fail-closed for RV until a dedicated gate exists.\n\
{} {{routine}}\n\
{}\n\
{{routine}}:\n\
\t// TODO: implement\n\
\tret\n",
                rules.export_directive, rules.text_section
            ),
            Isa::X86_64 => format!(
                "// {{routine}} — agent candidate skeleton\n\
{} {{routine}}\n\
{}\n\
{{routine}}:\n\
\t// TODO: implement\n\
\tret\n",
                rules.export_directive, rules.text_section
            ),
        },
    }
}

fn modeled_addressing_for(isa: Isa) -> Vec<String> {
    match isa {
        Isa::X86_64 => vec![
            "base+disp".to_string(),
            "base+index*scale+disp".to_string(),
            "rip-relative".to_string(),
            "rsp/rbp frame locals".to_string(),
        ],
        Isa::AArch64 => vec![
            "base+imm".to_string(),
            "base+index".to_string(),
            "pre/post-index".to_string(),
            "pc-relative adr/adrp".to_string(),
        ],
        Isa::Riscv64 | Isa::Riscv32 => vec![
            "base+imm12".to_string(),
            "auipc+offset".to_string(),
            "sp-relative frame locals".to_string(),
        ],
    }
}

fn supported_loop_idioms() -> Vec<String> {
    vec![
        "count-up induction with constant stride".to_string(),
        "countdown (dec) induction with exclusive bound".to_string(),
        "affine index patterns on single buffer leaves".to_string(),
    ]
}

fn known_incomplete_patterns_for(identity: &TargetIdentity) -> Vec<String> {
    let mut patterns = vec![
        "arbitrary control-flow invariants".to_string(),
        "general alias analysis / formal memory safety".to_string(),
        "full-ISA decode completeness certificate".to_string(),
    ];
    match identity.isa {
        Isa::X86_64 => {
            patterns.push("AVX/SSE vectorized buffer loops".to_string());
            patterns.push("indirect call/jump CFI proofs".to_string());
            if identity.object_format == ObjectFormat::PeCoff {
                patterns.push(
                    "Win64 varargs / floating-point home spills beyond integer ABI".to_string(),
                );
            } else {
                patterns.push("SysV red-zone use in non-leaf functions".to_string());
            }
        }
        Isa::AArch64 => {
            patterns.push("AArch64 write-shape region-precise store proof".to_string());
            patterns.push("NEON / SVE buffer idioms".to_string());
            patterns.push("PAC/BTI control-flow integrity".to_string());
        }
        Isa::Riscv64 | Isa::Riscv32 => {
            patterns.push(
                "RISC-V agent-verify sealed acceptance (fail-closed at VAA until dedicated gate)"
                    .to_string(),
            );
            patterns.push("compressed (C) extension completeness".to_string());
            patterns.push("vector extension buffer idioms".to_string());
        }
    }
    patterns
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn win64_profile_has_shadow_space_and_nasm() {
        let profile = AuthoringProfile::for_target("x86_64-pc-windows-msvc").unwrap();
        assert_eq!(profile.target, "x86_64-pc-windows-msvc");
        assert_eq!(profile.dialect, "nasm-intel");
        assert_eq!(profile.syntax, "nasm-intel");
        assert_eq!(profile.object_format, "pe-coff");
        assert_eq!(profile.abi.shadow_space_bytes, 32);
        assert_eq!(profile.abi.red_zone_bytes, 0);
        assert_eq!(profile.abi.stack_alignment, 16);
        assert_eq!(profile.abi.registers.parameter_registers[0], "rcx");
        assert!(profile.file_template.contains("BITS 64"));
        assert!(profile.profile_id.contains("x86_64-pc-windows-msvc"));
    }

    #[test]
    fn sysv_profile_has_red_zone_and_nasm() {
        let profile = AuthoringProfile::for_target("x86_64-unknown-linux-gnu").unwrap();
        assert_eq!(profile.target, "x86_64-unknown-linux-gnu");
        assert_eq!(profile.dialect, "nasm-intel");
        assert_eq!(profile.object_format, "elf");
        assert_eq!(profile.abi.shadow_space_bytes, 0);
        assert_eq!(profile.abi.red_zone_bytes, 128);
        assert_eq!(profile.abi.stack_alignment, 16);
        assert_eq!(profile.abi.registers.parameter_registers[0], "rdi");
        assert!(profile.file_template.contains("DEFAULT REL"));
    }

    #[test]
    fn rv_profile_documents_fail_closed_agent_verify() {
        let profile = AuthoringProfile::for_target("riscv64gc-unknown-linux-gnu").unwrap();
        assert_eq!(profile.dialect, "gas-unified");
        assert!(profile
            .known_incomplete_patterns
            .iter()
            .any(|p| p.contains("fail-closed")));
    }
}
