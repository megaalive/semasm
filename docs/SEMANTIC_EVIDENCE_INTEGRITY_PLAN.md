# Rekomendasi Teknis SemASM–VAA

## Milestone: Semantic Evidence Integrity

## 1. Keputusan utama

Untuk satu milestone berikutnya, bekukan sementara:

* penambahan ISA baru;
* penambahan routine/leaf baru;
* bridge generator baru;
* provider model baru;
* oracle baru yang tidak diperlukan untuk corpus existing;
* fitur signing atau transparency tambahan;
* klaim formal verification umum.

Fokuskan seluruh pekerjaan pada satu sasaran:

> Setiap status `assumed`, `required`, `observed`, `tested`, `proven`, dan `verified` harus memiliki arti yang berbeda, stabil, dan dapat dipaksakan oleh SemASM serta VAA.

Urutan prioritas:

1. **Perbaiki model alias dan caller obligation.**
2. **Tambahkan evidence requirement policy di VAA.**
3. **Bangun Region Access Evidence v1.**
4. **Perluas contract semantics secara terbatas.**
5. **Baru lanjutkan isolation dan trust-root hardening.**

---

# 2. P0 — Perbaiki makna alias dan precondition

## Masalah

Parameter pointer yang berbeda nama, misalnya `src` dan `dst`, tidak membuktikan bahwa alamat runtime-nya berbeda.

Pemanggilan berikut tetap mungkin:

```c
copy(p, p, length);
copy(p, p + 4, length);
```

Karena itu:

```text
different parameter names
```

tidak boleh secara otomatis menghasilkan:

```text
proven_disjoint
```

Model tersebut harus diperbaiki sebelum Region/Alias Evidence dipakai sebagai fondasi klaim lain.

## Model yang direkomendasikan

Pisahkan dua dimensi:

```text
relation result
+
evidence basis
```

### Relation result

```text
disjoint
equal
contains
partial_overlap
may_overlap
invalid
not_evaluated
```

### Evidence basis

```text
proven_static
declared_precondition
observed_runtime
behavioral_test
assumed_environment
unknown
```

Dengan demikian report dapat menyatakan:

```json
{
  "relation": {
    "left": "src",
    "right": "dst",
    "required": "disjoint",
    "observed": "may_overlap",
    "evidence_basis": "unknown",
    "status": "unresolved"
  }
}
```

Atau, bila kontrak memang mensyaratkan caller memberikan region terpisah:

```json
{
  "relation": {
    "left": "src",
    "right": "dst",
    "required": "disjoint",
    "observed": "disjoint",
    "evidence_basis": "declared_precondition",
    "status": "caller_obligation"
  }
}
```

## Perubahan schema kontrak

Gunakan deklarasi eksplisit:

```toml
[[function.memory.relations]]
left = "src"
right = "dst"
require = "disjoint"
basis = "precondition"
```

Alternatif yang lebih eksplisit:

```toml
[[function.preconditions]]
kind = "regions_disjoint"
left = "src"
right = "dst"
obligation = "caller"
```

Jangan menyimpulkan precondition dari nama parameter atau bentuk signature.

## Perubahan evaluator

Contract expression:

```text
regions.disjoint(src, dst)
```

tidak boleh menghasilkan `proven_true` hanya karena kontrak mendeklarasikannya sebagai precondition.

Gunakan hasil seperti:

```text
proven_true
proven_false
true_under_precondition
not_evaluated
incomplete
```

`true_under_precondition` berarti:

> SemASM dapat menganalisis implementasi dengan asumsi precondition tersebut, tetapi belum membuktikan bahwa setiap caller memenuhinya.

## Status verifikasi akhir

Pisahkan setidaknya:

```text
verified
verified_under_preconditions
incomplete
violated
failed
```

Contoh:

```json
{
  "status": "verified_under_preconditions",
  "unresolved_obligations": [
    {
      "kind": "regions_disjoint",
      "left": "src",
      "right": "dst",
      "owner": "caller"
    }
  ]
}
```

Jangan menurunkan seluruh implementasi menjadi gagal hanya karena mempunyai caller obligation yang sah. Namun jangan pula menyebutnya tanpa syarat sebagai `verified`.

## Test wajib

Tambahkan fixture berikut:

```text
different_pointer_names_are_not_proven_disjoint
same_pointer_is_proven_equal
same_base_overlapping_offsets_are_partial_overlap
explicit_disjoint_precondition_is_caller_obligation
precondition_does_not_become_proven_true
contradictory_relation_is_failed
unknown_pointer_arithmetic_is_incomplete
```

## Definition of done

Pekerjaan ini selesai bila:

* nama parameter tidak lagi menjadi bukti non-alias;
* precondition mempunyai representasi eksplisit;
* report membedakan bukti dan asumsi;
* evaluator tidak mengubah asumsi menjadi `proven_true`;
* VAA dapat melihat adanya unresolved caller obligation;
* dokumentasi menjelaskan bahwa callee verification tidak membuktikan caller compliance.

---

# 3. P1 — Evidence Requirement Profiles di VAA

## Tujuan

SemASM menentukan fakta teknis.

VAA menentukan fakta teknis mana yang wajib tersedia agar sebuah task dapat diterima.

Saat ini additive JSON compatibility berguna, tetapi VAA tidak boleh terus mengabaikan semantic evidence yang sebenarnya diwajibkan task.

## Jangan parse seluruh report SemASM

Pertahankan raw report untuk hashing dan forward compatibility.

Tambahkan typed projection yang tipis:

```rust
struct SemanticEvidenceSummary {
    alias: Option<AliasEvidenceSummary>,
    region_access: Option<RegionAccessSummary>,
    contract_expressions: Option<ContractExpressionSummary>,
    obligations: Vec<VerificationObligation>,
}
```

VAA tidak perlu memahami cara SemASM membuktikan fakta tersebut. VAA hanya memeriksa:

* evidence hadir;
* model dan versinya cocok;
* status memenuhi policy;
* tidak ada unknown yang dilarang;
* unresolved obligations sesuai kebijakan task.

## Schema task yang direkomendasikan

```toml
[verification.semantic_evidence.alias]
required = true
model = "region-alias-v1"
allow_incomplete = false
allow_caller_obligations = true

[verification.semantic_evidence.region_access]
required = true
model = "region-access-affine-v1"
allow_unknown_accesses = false

[verification.semantic_evidence.contract_expressions]
required = true
model = "contract-expr-v1"
allow_not_evaluated = false
```

Untuk task sederhana:

```toml
[verification.profile]
name = "leaf-pure-v1"
```

Untuk routine memori:

```toml
[verification.profile]
name = "memory-leaf-affine-v1"
```

Profile sebaiknya hanya menjadi ekspansi deterministik dari requirement individual. Locked task harus menyimpan hasil ekspansi tersebut agar perubahan definisi profile tidak mengubah task lama.

## Status VAA

Gunakan aturan:

```text
required evidence hilang       → incomplete
model/version tidak cocok      → failed
semantic contradiction         → violated
unknown yang dilarang policy   → incomplete
caller obligation diizinkan    → verified_under_preconditions
caller obligation dilarang     → incomplete
```

VAA tidak boleh mengubah `verified_under_preconditions` menjadi `verified` tanpa kebijakan eksplisit.

## Evidence checks

Tambahkan checks seperti:

```text
alias_evidence_present
alias_model_matches
alias_status_allowed
region_access_complete
contract_expressions_complete
caller_obligations_allowed
```

Checks tersebut harus ikut masuk ke:

* acceptance digest;
* evidence bundle;
* candidate chain;
* final seal.

## Compatibility

Gunakan version range eksplisit:

```text
region-alias-v1
region-access-affine-v1
contract-expr-v1
```

Unknown model:

```text
required unknown model → failed
optional unknown model → ignored tetapi dicatat
```

Jangan menggunakan aturan umum seperti “semua schema 0.x diterima”.

## Definition of done

* Task dapat mewajibkan semantic evidence tertentu.
* Evidence yang diwajibkan tetapi hilang tidak pernah menghasilkan `Verified`.
* Raw SemASM report tetap disimpan utuh.
* Checks baru ikut disegel.
* Resume dan verify-chain tetap menghasilkan keputusan yang sama.
* Fixture task lama tetap kompatibel atau mempunyai migrasi yang jelas.

---

# 4. P1 — Region Access Evidence v1

Alias relation saja belum cukup. SemASM juga harus menghubungkan setiap memory access yang dikenal dengan region kontrak.

## Scope yang diperbolehkan

Versi pertama hanya menangani alamat affine:

```text
base
base + constant
base + index
base + index * scale
base + affine offset sederhana
```

Dengan syarat nilai atau batas index dapat berasal dari kontrak yang dikenali.

Jangan mendukung dahulu:

* arbitrary pointer arithmetic;
* pointer hasil load dari memori;
* linked structures;
* heap provenance umum;
* nonlinear arithmetic;
* arbitrary interprocedural alias analysis;
* self-modifying code;
* arbitrary indirect memory effects.

## Model access

Setiap access minimal memiliki:

```rust
struct MemoryAccessEvidence {
    instruction_offset: u64,
    operation: LoadOrStore,
    width: u32,
    address_expression: AffineAddress,
    matched_region: Option<String>,
    bounds_status: BoundsStatus,
    permission_status: PermissionStatus,
}
```

### Bounds status

```text
proven_inside
proven_outside
may_escape
unknown
```

### Permission status

```text
allowed
denied
unknown
```

## Aturan fail-closed

```text
store ke region read-only      → violated
load dari region write-only    → violated atau policy-defined
akses pasti keluar bounds      → violated
akses mungkin keluar bounds    → incomplete
alamat tidak dapat dimodelkan  → incomplete
unknown instruction effect     → incomplete
```

Jangan mengubah akses unknown menjadi warning bila task mewajibkan complete region access evidence.

## Report

```json
{
  "region_access": {
    "model": "region-access-affine-v1",
    "status": "passed",
    "accesses_total": 8,
    "accesses_proven_inside": 8,
    "accesses_unknown": 0,
    "accesses": [
      {
        "instruction_offset": 12,
        "operation": "store",
        "width": 1,
        "address": "dst + index",
        "region": "dst",
        "bounds": "proven_inside",
        "permission": "allowed"
      }
    ]
  }
}
```

## Urutan implementasi

Jangan langsung mengaktifkan gate untuk semua target.

### Tahap pertama

* Engine target-neutral di ASIR/contract layer.
* Corpus x86-64 sebagai acceptance gate.
* AArch64/RV64 hanya menghasilkan report observasional.

### Tahap kedua

Setelah corpus x86 stabil:

* tambahkan fixture yang setara pada AArch64;
* tambahkan fixture yang setara pada RV64;
* baru naikkan capability masing-masing secara terpisah.

Jangan memakai status global “multi-ISA supported”.

## Corpus minimum

```text
load_inside_read_region
store_inside_write_region
store_to_read_only_region
load_before_region
store_after_region
unknown_base_register
known_base_unknown_offset
multi_byte_access_crosses_end
same_region_read_write
memcpy_disjoint_regions
memcpy_possible_overlap
```

## Definition of done

* Setiap known memory access mempunyai evidence.
* Unknown access dihitung secara eksplisit.
* Bounds dan permission dibedakan.
* Hasil terikat ke contract digest dan source digest.
* VAA dapat mewajibkan `unknown_accesses == 0`.
* Capability manifest menyebut target dan corpus yang benar-benar diuji.

---

# 5. P2 — Contract Expression Semantics v2

Kerjakan hanya setelah tiga bagian sebelumnya stabil.

## Tujuan

Memperluas contract semantics tanpa berubah menjadi theorem prover umum.

## Tambahan yang layak

```text
integer affine arithmetic
result bounds
length bounds
region length relations
offset constraints
simple implication
caller obligations
```

Contoh:

```text
length >= 0
result <= length
dst.length >= length
src.length >= length
index < length
```

## Jangan masukkan dahulu

```text
quantifier umum
loop invariant inference
recursive predicate
separation logic lengkap
SMT solver sebagai dependency wajib
symbolic execution umum
interprocedural whole-program proof
```

## Proof trace

Setiap hasil harus membawa basis:

```json
{
  "expression": "result <= length",
  "status": "proven_true",
  "basis": [
    "oracle.result_range",
    "contract.length_non_negative"
  ]
}
```

Bila evaluator hanya menerapkan literal constant folding:

```json
{
  "expression": "4 < 8",
  "status": "proven_true",
  "basis": [
    "closed_integer_evaluation"
  ]
}
```

Jangan menyebut evaluasi ekspresi tertutup sebagai formal proof atas perilaku program.

## Possible future solver

Jika kelak memakai SMT:

* solver harus optional adapter;
* input formula harus tersimpan;
* solver version harus dilaporkan;
* timeout harus menghasilkan `incomplete`;
* `unknown` solver tidak boleh menjadi `passed`;
* proof object atau reproducible query harus disimpan bila tersedia.

Namun SMT belum diperlukan untuk milestone sekarang.

---

# 6. P3 — Isolation operations proof

Prioritas ini naik menjadi P0 hanya bila VAA mulai:

* menjalankan kode pengguna publik;
* dipakai sebagai layanan bersama;
* berjalan di host penting;
* mempunyai akses credential atau jaringan;
* menerima candidate dari pihak yang tidak dipercaya.

Untuk penggunaan lokal terkendali, lanjutkan setelah semantic evidence stabil.

## Target berikutnya

Jangan mengejar label “secure sandbox”.

Bangun **Isolation Conformance Profile v1**:

```text
network disabled
capabilities dropped
no-new-privileges
read-only root
read-only inputs
ephemeral writable output
non-root user
PID limit
memory limit
CPU limit
device denial
no Docker socket
environment allow-list
process-tree termination
```

Report harus membedakan:

```text
requested
observed
enforced
not_available
not_verified
```

Contoh:

```json
{
  "network": {
    "requested": "disabled",
    "observed": "docker_arg_network_none",
    "enforced": "not_independently_verified"
  }
}
```

Command-line flag bukan bukti absolut kernel isolation. Pertahankan kejujuran itu.

---

# 7. P4 — Trust root nyata

Trust root dikerjakan setelah format acceptance evidence stabil.

Urutannya:

```text
content integrity
→ publisher signature
→ CI/workload identity
→ transparency inclusion
→ key policy
→ revocation and rotation
```

## Tahap yang disarankan

### Tahap pertama

* Release binaries ditandatangani melalui CI identity.
* Provenance mengikat source revision, workflow, SemASM pin, dan artefak.
* Transparency inclusion tersedia dan dapat diverifikasi.

### Tahap kedua

* Policy mengenai issuer dan repository identity.
* Key rotation atau keyless verification policy.
* Verifier offline yang jelas.
* Revocation/documented trust policy.

### Tetap nyatakan

```text
signature proves provenance/authenticity
signature does not prove semantic correctness
```

Jangan membuat signature atas evidence yang semantiknya masih ambigu.

---

# 8. Urutan commit yang direkomendasikan

## SemASM

```text
1. ADR/RFC: proof, assumption, obligation terminology
2. Tambah relation result dan evidence basis
3. Ubah distinct-pointer behavior menjadi may_overlap
4. Tambah explicit precondition schema
5. Ubah contract evaluator result model
6. Tambah report migration/schema version
7. Tambah positive/negative fixtures
8. Perbarui capability manifest
9. Implement Region Access engine
10. Aktifkan x86 acceptance corpus
11. Tambah AArch64/RV64 parity secara terpisah
```

## VAA

```text
1. Tambah typed semantic evidence projection
2. Tambah evidence requirement schema
3. Tambah built-in profile expansion
4. Lock expanded requirements ke task digest
5. Tambah semantic evidence checks
6. Tambah verified_under_preconditions
7. Ikat checks baru ke seal dan chain
8. Tambah migration fixtures
9. Tambah Gate CI untuk missing/incomplete/mismatched evidence
```

Jangan mengerjakan SemASM dan VAA dalam satu commit lintas-repository yang besar. Gunakan pin SemASM yang eksplisit pada VAA setelah setiap milestone SemASM stabil.

---

# 9. Release gating

## SemASM release berikutnya hanya boleh terbit bila

* distinct pointer names tidak lagi berarti proven disjoint;
* assumption dan proof dibedakan di schema/report;
* contract evaluator tidak mempromosikan assumption;
* Region Access x86 corpus hijau;
* unknown access fail-closed;
* capability manifest menyebut scope target secara spesifik;
* migration notes tersedia;
* golden demo menunjukkan `verified`, `verified_under_preconditions`, `incomplete`, dan `violated`.

## VAA release berikutnya hanya boleh terbit bila

* locked task dapat mewajibkan semantic evidence;
* missing required evidence gagal secara deterministik;
* model/version mismatch ditolak;
* caller obligations mengikuti policy task;
* evidence checks ikut acceptance digest;
* verify-bundle dan verify-chain tetap hijau;
* VAA mem-pin SemASM revision yang telah melalui gate tersebut.

---

# 10. Klaim publik yang boleh digunakan

## Sesudah P0–P1 selesai

> SemASM distinguishes statically proven region relations from declared caller preconditions.

> SemASM reports bounded affine region and memory-access evidence for supported leaf-routine patterns.

> VAA can require specific SemASM evidence models before accepting a candidate.

> A candidate may be accepted as verified under explicitly recorded preconditions.

## Klaim yang belum boleh digunakan

> SemASM proves general memory safety.

> SemASM performs complete alias analysis.

> SemASM formally verifies arbitrary assembly.

> VAA proves that every caller satisfies the callee contract.

> VAA securely executes arbitrary hostile native code.

> Signed evidence proves that the program is semantically correct.

---

# 11. Milestone akhir yang disarankan

Gunakan nama internal:

```text
Semantic Evidence Integrity
```

Deliverable utamanya:

```text
SemASM:
- Region/Alias Evidence v1.1
- Caller Obligations v1
- Region Access Evidence v1
- Contract Expression result model yang diperbaiki

VAA:
- Semantic Evidence Requirement Profiles v1
- verified_under_preconditions
- sealed semantic-evidence checks
```

Demo utamanya cukup satu routine:

```text
memcpy-like leaf routine
```

Empat kasus:

```text
1. Disjoint precondition declared
   → verified_under_preconditions

2. Exact alias tanpa izin
   → violated

3. Unknown address expression
   → incomplete

4. Store keluar destination region
   → violated
```

Satu demo tersebut akan menunjukkan bahwa SemASM–VAA tidak hanya mengenali instruksi dan menjalankan test vector, tetapi sudah mampu membedakan:

```text
apa yang dibuktikan
apa yang diasumsikan
apa yang diwajibkan caller
apa yang belum diketahui
apa yang benar-benar dilanggar
```

## Putusan akhir

Prioritas mutlak bukan menambah fitur baru.

Prioritas mutlak adalah:

> **memperbaiki vocabulary of truth di SemASM, kemudian membuat VAA mampu memaksakan vocabulary tersebut sebagai acceptance policy.**

Setelah lapisan itu stabil, Region Access Evidence menjadi langkah teknis berikutnya yang paling bernilai. Formal semantics, multi-ISA expansion, isolation proof, dan trust root kemudian dapat dibangun di atas fondasi yang tidak lagi mencampur asumsi dengan bukti.
