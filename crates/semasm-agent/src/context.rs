//! Context bundle generator.
//!
//! Builds a [`ContextBundle`](crate::ContextBundle) from a validated contract
//! and target identity, mapping ABI registers, classifying preserved vs
//! volatile registers, and producing deterministic output.

use std::fmt::Write;

use semasm_contract::CheckedContract;
use semasm_core::DiagnosticLevel;
use semasm_target::abi::ABIRegisterMap;
use semasm_target::TargetIdentity;

use crate::{ABIParameter, ABIReturn, ContextBundle, TargetToolchain, TestVector};

// ---------------------------------------------------------------------------
// Builder
// ---------------------------------------------------------------------------

impl ContextBundle {
    /// Build a `ContextBundle` from a validated contract and target identity.
    ///
    /// `existing_source` — content of the `.asm` file the agent should
    /// extend or modify (if any).
    ///
    /// `test_vectors` — externally supplied test cases (the generator does
    /// not synthesise them from contract constraints yet; see AGENT-004).
    ///
    /// `allowed_instructions` — explicit instruction set the agent may use;
    /// an empty vector means all instructions are permitted.
    #[must_use]
    #[allow(clippy::needless_pass_by_value)]
    pub fn generate(
        contract: &CheckedContract,
        target: &TargetIdentity,
        toolchain: &TargetToolchain,
        existing_source: Option<String>,
        test_vectors: Vec<TestVector>,
        allowed_instructions: Vec<String>,
    ) -> Self {
        let regs = target.abi_register_map();

        let abi_parameters = build_abi_params(contract, regs.as_ref());
        let abi_return = build_abi_return(contract, regs.as_ref());
        let (preserved, volatile) = registers(regs.as_ref());

        ContextBundle {
            function_name: contract.name.clone(),
            abi_parameters,
            abi_return,
            preserved_registers: preserved,
            volatile_registers: volatile,
            allowed_instructions,
            existing_source,
            test_vectors,
            acceptance_commands: build_acceptance(target, toolchain),
        }
    }

    /// Render the bundle as a human-readable Markdown string.
    #[must_use]
    pub fn to_markdown(&self) -> String {
        let mut md = String::new();

        let _ = writeln!(md, "# Context bundle: `{}`\n", self.function_name);

        md.push_str("## ABI parameters\n\n");
        md.push_str("| # | Name | Register | Type |\n");
        md.push_str("|---|---|---|---|\n");
        for (i, p) in self.abi_parameters.iter().enumerate() {
            let _ = writeln!(
                md,
                "| {i} | `{}` | `{}` | `{}` |",
                p.name, p.register, p.type_name
            );
        }
        md.push('\n');

        let _ = write!(
            md,
            "## Return value\n\n| Register | Type |\n|---|---|\n| `{}` | `{}` |\n\n",
            self.abi_return.register, self.abi_return.type_name,
        );

        md.push_str("## Preserved registers (callee-saved)\n\n```\n");
        for r in &self.preserved_registers {
            let _ = writeln!(md, "{r}");
        }
        md.push_str("```\n\n");

        md.push_str("## Volatile registers (caller-saved)\n\n```\n");
        for r in &self.volatile_registers {
            let _ = writeln!(md, "{r}");
        }
        md.push_str("```\n\n");

        if !self.allowed_instructions.is_empty() {
            md.push_str("## Allowed instructions\n\n```\n");
            for i in &self.allowed_instructions {
                let _ = writeln!(md, "{i}");
            }
            md.push_str("```\n\n");
        }

        if let Some(src) = &self.existing_source {
            md.push_str("## Existing source\n\n```asm\n");
            md.push_str(src);
            md.push_str("\n```\n\n");
        }

        if !self.test_vectors.is_empty() {
            md.push_str("## Test vectors\n\n");
            for tv in &self.test_vectors {
                let _ = writeln!(md, "- **{}**  ", tv.name);
                let _ = writeln!(md, "  inputs: `{}`  ", json_array(&tv.inputs));
                let _ = writeln!(md, "  expected: `{}`\n", tv.expected);
            }
        }

        if !self.acceptance_commands.is_empty() {
            md.push_str("## Acceptance commands\n\n```bash\n");
            for cmd in &self.acceptance_commands {
                let _ = writeln!(md, "{cmd}");
            }
            md.push_str("```\n");
        }

        md
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn build_abi_params(
    contract: &CheckedContract,
    regs: Option<&ABIRegisterMap>,
) -> Vec<ABIParameter> {
    contract
        .parameters
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let register = regs
                .and_then(|r| r.param_register(i))
                .unwrap_or("?")
                .to_string();
            ABIParameter {
                name: p.name.clone(),
                register,
                type_name: p.type_source.clone(),
            }
        })
        .collect()
}

fn build_abi_return(contract: &CheckedContract, regs: Option<&ABIRegisterMap>) -> ABIReturn {
    let register = regs.map_or_else(|| "?".to_string(), |r| r.return_register.clone());
    let type_name = contract
        .returns
        .first()
        .map_or_else(|| "void".to_string(), |r| r.type_source.clone());
    ABIReturn {
        register,
        type_name,
    }
}

fn registers(regs: Option<&ABIRegisterMap>) -> (Vec<String>, Vec<String>) {
    match regs {
        Some(r) => (r.preserved_registers.clone(), r.volatile_registers.clone()),
        None => (vec![], vec![]),
    }
}

fn build_acceptance(_target: &TargetIdentity, toolchain: &TargetToolchain) -> Vec<String> {
    // Derived from the build pipeline known in semasm-build.
    // These are reasonable defaults for the first-slice target.
    let fmt = "elf64";
    let ext = "asm";
    vec![
        format!("{} -f {fmt} -o /dev/null src/*.{ext}", toolchain.assembler),
        format!(
            "{} --build-id=none --hash-style=sysv -o /dev/null /dev/null",
            toolchain.linker,
        ),
        format!("{} -f {fmt} src/*.{ext} -o src/*.o", toolchain.assembler),
    ]
}

fn json_array(values: &[serde_json::Value]) -> String {
    let parts: Vec<String> = values.iter().map(|v| format!("{v}")).collect();
    format!("[{}]", parts.join(", "))
}

// ---------------------------------------------------------------------------
// Diagnostics integration
// ---------------------------------------------------------------------------

/// Warnings and errors emitted during context generation.
#[derive(Debug, Clone)]
pub struct ContextDiagnostic {
    /// Severity level.
    pub level: DiagnosticLevel,
    /// Human-readable explanation.
    pub message: String,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{sample_check, sample_target, sample_toolchain};

    #[test]
    fn generates_abi_parameters_for_sysv() {
        let contract = sample_check();
        let target = sample_target();
        let bundle = ContextBundle::generate(
            &contract,
            &target,
            &sample_toolchain(),
            None,
            vec![],
            vec![],
        );

        assert_eq!(bundle.function_name, "count_byte");
        assert_eq!(bundle.abi_parameters.len(), 3);
        assert_eq!(bundle.abi_parameters[0].register, "rdi");
        assert_eq!(bundle.abi_parameters[1].register, "rsi");
        assert_eq!(bundle.abi_parameters[2].register, "rdx");
    }

    #[test]
    fn generates_return_register() {
        let contract = sample_check();
        let target = sample_target();
        let bundle = ContextBundle::generate(
            &contract,
            &target,
            &sample_toolchain(),
            None,
            vec![],
            vec![],
        );

        assert_eq!(bundle.abi_return.register, "rax");
    }

    #[test]
    fn generates_preserved_registers() {
        let contract = sample_check();
        let target = sample_target();
        let bundle = ContextBundle::generate(
            &contract,
            &target,
            &sample_toolchain(),
            None,
            vec![],
            vec![],
        );

        assert!(bundle.preserved_registers.contains(&"rbx".to_string()));
        assert!(bundle.preserved_registers.contains(&"r12".to_string()));
    }

    #[test]
    fn generates_volatile_registers() {
        let contract = sample_check();
        let target = sample_target();
        let bundle = ContextBundle::generate(
            &contract,
            &target,
            &sample_toolchain(),
            None,
            vec![],
            vec![],
        );

        assert!(bundle.volatile_registers.contains(&"rax".to_string()));
        assert!(bundle.volatile_registers.contains(&"r11".to_string()));
    }

    #[test]
    fn preserved_and_volatile_do_not_overlap() {
        let contract = sample_check();
        let target = sample_target();
        let bundle = ContextBundle::generate(
            &contract,
            &target,
            &sample_toolchain(),
            None,
            vec![],
            vec![],
        );

        for p in &bundle.preserved_registers {
            assert!(
                !bundle.volatile_registers.contains(p),
                "{p} appears in both preserved and volatile sets"
            );
        }
    }

    #[test]
    fn markdown_output_includes_function_name() {
        let contract = sample_check();
        let target = sample_target();
        let bundle = ContextBundle::generate(
            &contract,
            &target,
            &sample_toolchain(),
            None,
            vec![],
            vec![],
        );

        let md = bundle.to_markdown();
        assert!(md.contains("count_byte"));
        assert!(md.contains("rdi"));
        assert!(md.contains("rbx"));
        assert!(md.contains("## ABI parameters"));
    }

    #[test]
    fn markdown_includes_test_vectors() {
        let contract = sample_check();
        let target = sample_target();
        let tv = TestVector {
            name: "empty".into(),
            inputs: vec![serde_json::Value::Null],
            expected: serde_json::Value::Number(0.into()),
        };
        let bundle = ContextBundle::generate(
            &contract,
            &target,
            &sample_toolchain(),
            None,
            vec![tv],
            vec![],
        );

        let md = bundle.to_markdown();
        assert!(md.contains("empty"));
        assert!(md.contains("## Test vectors"));
    }

    #[test]
    fn markdown_includes_existing_source() {
        let contract = sample_check();
        let target = sample_target();
        let bundle = ContextBundle::generate(
            &contract,
            &target,
            &sample_toolchain(),
            Some("; hello world\n".into()),
            vec![],
            vec![],
        );

        let md = bundle.to_markdown();
        assert!(md.contains("hello world"));
        assert!(md.contains("## Existing source"));
    }

    #[test]
    fn deterministic_across_calls() {
        let contract = sample_check();
        let target = sample_target();
        let a = ContextBundle::generate(
            &contract,
            &target,
            &sample_toolchain(),
            None,
            vec![],
            vec![],
        );
        let b = ContextBundle::generate(
            &contract,
            &target,
            &sample_toolchain(),
            None,
            vec![],
            vec![],
        );
        assert_eq!(a, b, "context bundle must be deterministic");
    }
}
