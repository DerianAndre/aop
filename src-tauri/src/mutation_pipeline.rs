use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use tokio::process::Command;
use tokio::time::timeout;
use uuid::Uuid;

use crate::db::metrics;
use crate::db::mutations::{self, MutationRecord, MutationStatus, UpdateMutationStatusInput};
use crate::db::tasks::{self, TaskRecord, TaskStatus, UpdateTaskOutcomeInput};
use crate::vector::indexer;
use crate::vector::indexer::embed_text;

const SHADOW_TIMEOUT: Duration = Duration::from_secs(120);
const APPLY_TIMEOUT: Duration = Duration::from_secs(60);
const SEMANTIC_THRESHOLD: f32 = 0.08;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunMutationPipelineInput {
    pub mutation_id: String,
    pub target_project: String,
    pub tier1_approved: bool,
    pub ci_command: Option<String>,
    pub ci_args: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PipelineStepResult {
    pub step: String,
    pub status: String,
    pub details: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MutationPipelineResult {
    pub mutation: MutationRecord,
    pub task: TaskRecord,
    pub steps: Vec<PipelineStepResult>,
    pub shadow_dir: Option<String>,
}

#[derive(Debug, Clone)]
struct ShadowOutcome {
    status: MutationStatus,
    test_result: String,
    test_exit_code: Option<i64>,
    shadow_dir: PathBuf,
}

#[derive(Debug, Clone)]
struct CommandResult {
    exit_code: i32,
    stdout: String,
    stderr: String,
}

#[derive(Debug, Clone)]
enum CiPlan {
    Command {
        program: String,
        args: Vec<String>,
        label: String,
    },
    NoTests,
}

pub async fn run_mutation_pipeline(
    pool: &SqlitePool,
    input: RunMutationPipelineInput,
) -> Result<MutationPipelineResult, String> {
    validate_input(&input)?;
    let mutation = mutations::get_mutation_by_id(pool, input.mutation_id.trim()).await?;
    let task = tasks::get_task_by_id(pool, mutation.task_id.trim()).await?;
    let mut steps: Vec<PipelineStepResult> = Vec::new();

    if mutation.status == MutationStatus::Applied.as_str() {
        return Err(format!("Mutation '{}' is already applied.", mutation.id));
    }
    if mutation.status == MutationStatus::Rejected.as_str() {
        return Err(format!("Mutation '{}' is already rejected.", mutation.id));
    }

    metrics::record_audit_event(
        pool,
        "mutation_pipeline",
        "pipeline_started",
        Some(&mutation.id),
        Some(&format!("{{\"taskId\":\"{}\"}}", task.id)),
    )
    .await?;

    let shadow = match run_shadow_test(&mutation, &input).await {
        Ok(value) => {
            steps.push(PipelineStepResult {
                step: "shadow_test".to_string(),
                status: "passed".to_string(),
                details: value.test_result.clone(),
            });
            value
        }
        Err(error) => {
            steps.push(PipelineStepResult {
                step: "shadow_test".to_string(),
                status: "failed".to_string(),
                details: error.clone(),
            });
            return reject_pipeline(
                pool,
                mutation,
                task,
                steps,
                "shadow_test",
                &error,
                None,
                None,
            )
            .await;
        }
    };

    let semantic_score = match semantic_similarity_score(&mutation, &shadow.shadow_dir) {
        Ok(value) => value,
        Err(error) => {
            steps.push(PipelineStepResult {
                step: "semantic_regression".to_string(),
                status: "failed".to_string(),
                details: error.clone(),
            });
            return reject_pipeline(
                pool,
                mutation,
                task,
                steps,
                "semantic_regression",
                &error,
                Some(shadow.test_result),
                shadow.test_exit_code,
            )
            .await;
        }
    };

    if semantic_score < SEMANTIC_THRESHOLD {
        let message = format!(
            "Intent similarity {:.3} is below threshold {:.3}.",
            semantic_score, SEMANTIC_THRESHOLD
        );
        steps.push(PipelineStepResult {
            step: "semantic_regression".to_string(),
            status: "failed".to_string(),
            details: message.clone(),
        });
        return reject_pipeline(
            pool,
            mutation,
            task,
            steps,
            "semantic_regression",
            &message,
            Some(shadow.test_result),
            shadow.test_exit_code,
        )
        .await;
    }

    steps.push(PipelineStepResult {
        step: "semantic_regression".to_string(),
        status: "passed".to_string(),
        details: format!("Intent similarity {:.3}.", semantic_score),
    });

    if let Err(error) = run_tier2_compliance_check(&task, &mutation) {
        steps.push(PipelineStepResult {
            step: "tier2_compliance".to_string(),
            status: "failed".to_string(),
            details: error.clone(),
        });
        return reject_pipeline(
            pool,
            mutation,
            task,
            steps,
            "tier2_compliance",
            &error,
            Some(shadow.test_result),
            shadow.test_exit_code,
        )
        .await;
    }

    steps.push(PipelineStepResult {
        step: "tier2_compliance".to_string(),
        status: "passed".to_string(),
        details: "Compliance checks passed.".to_string(),
    });

    let mut updated_mutation = mutations::update_mutation_status(
        pool,
        UpdateMutationStatusInput {
            mutation_id: mutation.id.clone(),
            status: shadow.status,
            test_result: Some(shadow.test_result.clone()),
            test_exit_code: shadow.test_exit_code,
            rejection_reason: None,
            rejected_at_step: None,
        },
    )
    .await?;

    steps.push(PipelineStepResult {
        step: "validation_status".to_string(),
        status: "passed".to_string(),
        details: format!("Mutation marked as {}.", updated_mutation.status),
    });

    if !input.tier1_approved {
        let updated_task = tasks::update_task_outcome(
            pool,
            UpdateTaskOutcomeInput {
                task_id: task.id.clone(),
                status: TaskStatus::Paused,
                token_usage: None,
                context_efficiency_ratio: None,
                compliance_score: Some(70),
                checksum_before: None,
                checksum_after: None,
                error_message: Some("Waiting for Tier 1 approval before apply.".to_string()),
            },
        )
        .await?;

        steps.push(PipelineStepResult {
            step: "tier1_final_approval".to_string(),
            status: "pending".to_string(),
            details: "Validation complete. Tier 1 approval required.".to_string(),
        });

        return Ok(MutationPipelineResult {
            mutation: updated_mutation,
            task: updated_task,
            steps,
            shadow_dir: Some(shadow.shadow_dir.to_string_lossy().to_string()),
        });
    }

    steps.push(PipelineStepResult {
        step: "tier1_final_approval".to_string(),
        status: "passed".to_string(),
        details: "Tier 1 approval granted.".to_string(),
    });

    let checksum_before =
        checksum_for_target_file(&input.target_project, &updated_mutation.file_path)?;
    let apply_details =
        match apply_and_commit_mutation(&input.target_project, &updated_mutation).await {
            Ok(value) => value,
            Err(error) => {
                steps.push(PipelineStepResult {
                    step: "apply".to_string(),
                    status: "failed".to_string(),
                    details: error.clone(),
                });
                return reject_pipeline(
                    pool,
                    updated_mutation,
                    task,
                    steps,
                    "apply",
                    &error,
                    Some(shadow.test_result),
                    shadow.test_exit_code,
                )
                .await;
            }
        };

    steps.push(PipelineStepResult {
        step: "apply".to_string(),
        status: "passed".to_string(),
        details: apply_details,
    });

    let checksum_after =
        checksum_for_target_file(&input.target_project, &updated_mutation.file_path)?;
    updated_mutation = mutations::update_mutation_status(
        pool,
        UpdateMutationStatusInput {
            mutation_id: updated_mutation.id.clone(),
            status: MutationStatus::Applied,
            test_result: Some(shadow.test_result),
            test_exit_code: shadow.test_exit_code,
            rejection_reason: None,
            rejected_at_step: None,
        },
    )
    .await?;

    let updated_task = tasks::update_task_outcome(
        pool,
        UpdateTaskOutcomeInput {
            task_id: task.id,
            status: TaskStatus::Completed,
            token_usage: Some(task.token_usage.saturating_add(450)),
            context_efficiency_ratio: Some(1.0),
            compliance_score: Some(85),
            checksum_before: Some(checksum_before),
            checksum_after: Some(checksum_after),
            error_message: None,
        },
    )
    .await?;

    let _ = indexer::index_project(pool, &input.target_project).await;

    Ok(MutationPipelineResult {
        mutation: updated_mutation,
        task: updated_task,
        steps,
        shadow_dir: Some(shadow.shadow_dir.to_string_lossy().to_string()),
    })
}

fn validate_input(input: &RunMutationPipelineInput) -> Result<(), String> {
    if input.mutation_id.trim().is_empty() {
        return Err("mutationId is required".to_string());
    }
    if input.target_project.trim().is_empty() {
        return Err("targetProject is required".to_string());
    }

    Ok(())
}

async fn run_shadow_test(
    mutation: &MutationRecord,
    input: &RunMutationPipelineInput,
) -> Result<ShadowOutcome, String> {
    let target_root = normalize_target_root(&input.target_project)?;
    let shadow_root = create_shadow_dir()?;
    copy_project_for_shadow(&target_root, &shadow_root)?;

    let patch_content = normalize_patch_line_endings(&mutation.diff_content);
    validate_patch_format(&patch_content)?;

    let patch_path = shadow_root.join("aop_mutation.patch");
    fs::write(&patch_path, &patch_content)
        .map_err(|error| format!("Failed to write patch in shadow dir: {error}"))?;
    let patch_value = patch_path.to_string_lossy().to_string();

    normalize_file_line_endings_in_dir(&shadow_root, &mutation.file_path)?;

    run_command(&shadow_root, "git", &["init", "-q"], SHADOW_TIMEOUT).await?;
    run_command(
        &shadow_root,
        "git",
        &["apply", "--check", "--whitespace=nowarn", patch_value.as_str()],
        SHADOW_TIMEOUT,
    )
    .await?;
    run_command(
        &shadow_root,
        "git",
        &["apply", "--whitespace=nowarn", patch_value.as_str()],
        SHADOW_TIMEOUT,
    )
    .await?;

    let ci_plan = detect_ci_plan(
        &shadow_root,
        input.ci_command.as_deref(),
        input.ci_args.clone(),
    )?;
    let (status, test_result, test_exit_code) = match ci_plan {
        CiPlan::NoTests => (
            MutationStatus::ValidatedNoTests,
            "No automated tests detected. Marked as validated_no_tests.".to_string(),
            None,
        ),
        CiPlan::Command {
            program,
            args,
            label,
        } => {
            let result = run_command_owned(&shadow_root, &program, args, SHADOW_TIMEOUT).await?;
            (
                MutationStatus::Validated,
                format!("{label} passed (exit code {}).", result.exit_code),
                Some(i64::from(result.exit_code)),
            )
        }
    };

    Ok(ShadowOutcome {
        status,
        test_result,
        test_exit_code,
        shadow_dir: shadow_root,
    })
}

fn detect_ci_plan(
    root: &Path,
    override_command: Option<&str>,
    override_args: Option<Vec<String>>,
) -> Result<CiPlan, String> {
    if let Some(command) = override_command
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Ok(CiPlan::Command {
            program: command.to_string(),
            args: override_args.unwrap_or_default(),
            label: "override_ci_command".to_string(),
        });
    }

    let package_json = root.join("package.json");
    if package_json.exists() {
        let raw = fs::read_to_string(package_json)
            .map_err(|error| format!("Failed to read package.json: {error}"))?;
        let parsed = serde_json::from_str::<serde_json::Value>(&raw)
            .map_err(|error| format!("Failed to parse package.json: {error}"))?;
        let has_test_script = parsed
            .get("scripts")
            .and_then(|value| value.get("test"))
            .and_then(|value| value.as_str())
            .map(|value| !value.trim().is_empty())
            .unwrap_or(false);

        if has_test_script {
            if root.join("node_modules").exists() {
                return Ok(CiPlan::Command {
                    program: "pnpm".to_string(),
                    args: vec!["test".to_string()],
                    label: "pnpm test".to_string(),
                });
            }

            return Ok(CiPlan::NoTests);
        }
    }

    if root.join("Cargo.toml").exists() {
        return Ok(CiPlan::Command {
            program: "cargo".to_string(),
            args: vec!["test".to_string(), "--quiet".to_string()],
            label: "cargo test --quiet".to_string(),
        });
    }

    Ok(CiPlan::NoTests)
}

fn semantic_similarity_score(mutation: &MutationRecord, shadow_root: &Path) -> Result<f32, String> {
    let target_file = resolve_target_file(shadow_root, &mutation.file_path)?;
    let content = fs::read_to_string(target_file).unwrap_or_default();
    let preview = content.chars().take(1200).collect::<String>();
    let before = mutation
        .intent_description
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| mutation.diff_content.clone());
    let after = format!("{} {}", mutation.file_path, preview);

    let left = embed_text(&before);
    let right = embed_text(&after);
    Ok(cosine_similarity(&left, &right))
}

fn run_tier2_compliance_check(task: &TaskRecord, mutation: &MutationRecord) -> Result<(), String> {
    let extension = Path::new(&mutation.file_path)
        .extension()
        .and_then(OsStr::to_str)
        .unwrap_or_default()
        .to_ascii_lowercase();
    let allowed = [
        "ts", "tsx", "js", "jsx", "rs", "json", "css", "md", "py", "go", "java", "toml",
    ];
    if !allowed.iter().any(|value| *value == extension) {
        return Err(format!(
            "File extension '.{}' is not allowed by compliance rules.",
            extension
        ));
    }

    let diff = mutation.diff_content.to_ascii_lowercase();
    if diff.contains("<<<<<<<") || diff.contains(">>>>>>>") {
        return Err("Diff contains unresolved conflict markers.".to_string());
    }
    if task.domain == "auth"
        && (diff.contains("bypass") || diff.contains("disable_auth") || diff.contains("skip auth"))
    {
        return Err("Auth mutation appears to bypass authentication controls.".to_string());
    }
    if task.domain == "database" && (diff.contains("drop table") || diff.contains("truncate ")) {
        return Err("Database mutation contains destructive statements.".to_string());
    }

    Ok(())
}

async fn apply_and_commit_mutation(
    target_project: &str,
    mutation: &MutationRecord,
) -> Result<String, String> {
    let target_root = normalize_target_root(target_project)?;
    if !target_root.join(".git").exists() {
        return Err(format!(
            "Target project '{}' is not a git repository (.git missing).",
            target_root.display()
        ));
    }

    let patch_content = normalize_patch_line_endings(&mutation.diff_content);
    let patch_path = target_root.join(format!(".aop_apply_{}.patch", mutation.id));
    fs::write(&patch_path, &patch_content)
        .map_err(|error| format!("Failed to write apply patch file: {error}"))?;
    let patch_value = patch_path.to_string_lossy().to_string();

    normalize_file_line_endings_in_dir(&target_root, &mutation.file_path)?;

    run_command(
        &target_root,
        "git",
        &["apply", "--check", "--whitespace=nowarn", patch_value.as_str()],
        APPLY_TIMEOUT,
    )
    .await?;
    run_command(
        &target_root,
        "git",
        &["apply", "--whitespace=nowarn", patch_value.as_str()],
        APPLY_TIMEOUT,
    )
    .await?;
    let _ = fs::remove_file(&patch_path);

    let auto_commit = std::env::var("AOP_AUTO_COMMIT_MUTATIONS")
        .map(|v| matches!(v.trim(), "1" | "true" | "yes" | "on"))
        .unwrap_or(false);

    if auto_commit {
        run_command_owned(
            &target_root,
            "git",
            vec!["add".to_string(), mutation.file_path.clone()],
            APPLY_TIMEOUT,
        )
        .await?;
        run_command_owned(
            &target_root,
            "git",
            vec![
                "commit".to_string(),
                "-m".to_string(),
                format!("chore(aop): apply mutation {}", mutation.id),
            ],
            APPLY_TIMEOUT,
        )
        .await?;

        Ok(format!(
            "Patch applied and committed for '{}'.",
            mutation.file_path
        ))
    } else {
        Ok(format!(
            "Patch applied for '{}' (auto-commit disabled).",
            mutation.file_path
        ))
    }
}

fn checksum_for_target_file(
    target_project: &str,
    relative_file_path: &str,
) -> Result<String, String> {
    let root = normalize_target_root(target_project)?;
    let target_file = resolve_target_file(&root, relative_file_path)?;
    if !target_file.exists() {
        // New file — no content to checksum yet
        return Ok("new_file".to_string());
    }
    let bytes = fs::read(target_file)
        .map_err(|error| format!("Failed to read target file for checksum: {error}"))?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

fn normalize_target_root(target_project: &str) -> Result<PathBuf, String> {
    let root = PathBuf::from(target_project.trim());
    let normalized = strip_unc_prefix(
        fs::canonicalize(root)
            .map_err(|error| format!("Unable to resolve target project path: {error}"))?,
    );
    if !normalized.is_dir() {
        return Err(format!(
            "Target project path '{}' is not a directory.",
            normalized.display()
        ));
    }

    Ok(normalized)
}

/// Strip the Windows extended-length path prefix (`\\?\`) that `fs::canonicalize` adds.
/// Git and most external tools cannot handle UNC paths.
fn strip_unc_prefix(path: PathBuf) -> PathBuf {
    let s = path.to_string_lossy();
    if let Some(stripped) = s.strip_prefix(r"\\?\") {
        PathBuf::from(stripped)
    } else {
        path
    }
}

fn resolve_target_file(project_root: &Path, relative_file_path: &str) -> Result<PathBuf, String> {
    if relative_file_path.trim().is_empty() {
        return Err("mutation file path is empty".to_string());
    }
    if relative_file_path.contains("..") {
        return Err("mutation file path cannot contain '..'".to_string());
    }

    let path = relative_file_path
        .split('/')
        .filter(|part| !part.is_empty())
        .fold(project_root.to_path_buf(), |acc, part| acc.join(part));

    // For existing files, canonicalize to verify the path stays within project root.
    // For new files (created by git apply), skip canonicalize since the file doesn't
    // exist yet — just verify the constructed path starts with the project root.
    if path.exists() {
        let canonicalized = strip_unc_prefix(
            fs::canonicalize(&path)
                .map_err(|error| format!("Failed to resolve mutation file '{}': {error}", path.display()))?,
        );
        let canonical_root = strip_unc_prefix(
            fs::canonicalize(project_root).unwrap_or_else(|_| project_root.to_path_buf()),
        );
        if !canonicalized.starts_with(&canonical_root) {
            return Err("mutation file path escapes target project root".to_string());
        }
        Ok(canonicalized)
    } else {
        // New file — verify the path doesn't escape project root by checking prefix
        if !path.starts_with(project_root) {
            return Err("mutation file path escapes target project root".to_string());
        }
        Ok(path)
    }
}

fn create_shadow_dir() -> Result<PathBuf, String> {
    let dir = std::env::temp_dir().join(format!("aop_shadow_{}", Uuid::new_v4()));
    fs::create_dir_all(&dir).map_err(|error| {
        format!(
            "Failed to create shadow directory '{}': {error}",
            dir.display()
        )
    })?;
    Ok(dir)
}

fn copy_project_for_shadow(source_root: &Path, destination_root: &Path) -> Result<(), String> {
    let mut stack = vec![source_root.to_path_buf()];

    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).map_err(|error| {
            format!(
                "Failed to read source directory '{}': {error}",
                dir.display()
            )
        })? {
            let entry = entry.map_err(|error| format!("Failed to read source entry: {error}"))?;
            let file_type = entry.file_type().map_err(|error| {
                format!("Failed to inspect '{}': {error}", entry.path().display())
            })?;
            let entry_path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();

            if is_windows_reserved_name(&name) {
                continue;
            }

            if file_type.is_dir() {
                if should_skip_shadow_dir(&name) {
                    continue;
                }

                let relative = entry_path.strip_prefix(source_root).map_err(|error| {
                    format!("Failed to compute shadow relative directory path: {error}")
                })?;
                fs::create_dir_all(destination_root.join(relative)).map_err(|error| {
                    format!("Failed to create shadow destination directory: {error}")
                })?;
                stack.push(entry_path);
            } else if file_type.is_file() {
                let src = entry_path;
                let relative = src.strip_prefix(source_root).map_err(|error| {
                    format!("Failed to compute shadow relative file path: {error}")
                })?;
                let dst = destination_root.join(relative);
                if let Some(parent) = dst.parent() {
                    fs::create_dir_all(parent)
                        .map_err(|error| format!("Failed to create destination parent: {error}"))?;
                }
                fs::copy(&src, &dst).map_err(|error| {
                    format!(
                        "Failed to copy '{}' to '{}' for shadow testing: {error}",
                        src.display(),
                        dst.display()
                    )
                })?;
            }
        }
    }

    Ok(())
}

/// Windows reserved device names that cannot be used as file names.
/// Trying to copy these causes OS error 87 ("The parameter is incorrect").
fn is_windows_reserved_name(name: &str) -> bool {
    let stem = Path::new(name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(name);
    matches!(
        stem.to_ascii_uppercase().as_str(),
        "CON" | "PRN" | "AUX" | "NUL"
            | "COM0" | "COM1" | "COM2" | "COM3" | "COM4"
            | "COM5" | "COM6" | "COM7" | "COM8" | "COM9"
            | "LPT0" | "LPT1" | "LPT2" | "LPT3" | "LPT4"
            | "LPT5" | "LPT6" | "LPT7" | "LPT8" | "LPT9"
    )
}

fn should_skip_shadow_dir(name: &str) -> bool {
    matches!(
        name,
        ".git" | "node_modules" | "target" | "dist" | "build" | ".next" | ".turbo"
    )
}

async fn run_command(
    working_dir: &Path,
    program: &str,
    args: &[&str],
    timeout_duration: Duration,
) -> Result<CommandResult, String> {
    run_command_owned(
        working_dir,
        program,
        args.iter().map(|value| (*value).to_string()).collect(),
        timeout_duration,
    )
    .await
}

async fn run_command_owned(
    working_dir: &Path,
    program: &str,
    args: Vec<String>,
    timeout_duration: Duration,
) -> Result<CommandResult, String> {
    let mut command = Command::new(program);
    command.current_dir(working_dir);
    for arg in &args {
        command.arg(arg);
    }

    let output = timeout(timeout_duration, command.output())
        .await
        .map_err(|_| {
            format!(
                "Command '{program} {}' timed out after {} seconds.",
                args.join(" "),
                timeout_duration.as_secs()
            )
        })?
        .map_err(|error| format!("Failed to run command '{program}': {error}"))?;

    let result = CommandResult {
        exit_code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    };
    if !output.status.success() {
        return Err(format!(
            "Command '{program} {}' failed with exit code {}.\nstdout:\n{}\nstderr:\n{}",
            args.join(" "),
            result.exit_code,
            result.stdout,
            result.stderr
        ));
    }

    Ok(result)
}

fn normalize_patch_line_endings(patch: &str) -> String {
    let normalized = patch.replace("\r\n", "\n");
    if normalized.ends_with('\n') {
        normalized
    } else {
        format!("{normalized}\n")
    }
}

fn validate_patch_format(patch: &str) -> Result<(), String> {
    if patch.trim().is_empty() {
        return Err("Patch content is empty.".to_string());
    }

    let has_header = patch.contains("--- ") && patch.contains("+++ ");
    if !has_header {
        return Err("Patch is missing unified diff headers (--- / +++)".to_string());
    }

    let has_hunk = patch.contains("@@ ");
    if !has_hunk {
        return Err("Patch is missing hunk headers (@@)".to_string());
    }

    Ok(())
}

fn normalize_file_line_endings_in_dir(root: &Path, relative_file_path: &str) -> Result<(), String> {
    let file_path = relative_file_path
        .split('/')
        .filter(|part| !part.is_empty())
        .fold(root.to_path_buf(), |acc, part| acc.join(part));

    if !file_path.exists() {
        return Ok(());
    }

    let content = fs::read_to_string(&file_path)
        .map_err(|error| format!("Failed to read file for line ending normalization: {error}"))?;

    if content.contains("\r\n") {
        let normalized = content.replace("\r\n", "\n");
        fs::write(&file_path, normalized).map_err(|error| {
            format!("Failed to write normalized file: {error}")
        })?;
    }

    Ok(())
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.is_empty() || b.is_empty() || a.len() != b.len() {
        return 0.0;
    }

    let dot = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum::<f32>();
    let norm_a = a.iter().map(|value| value * value).sum::<f32>().sqrt();
    let norm_b = b.iter().map(|value| value * value).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }

    dot / (norm_a * norm_b)
}

async fn reject_pipeline(
    pool: &SqlitePool,
    mutation: MutationRecord,
    task: TaskRecord,
    steps: Vec<PipelineStepResult>,
    rejected_step: &str,
    reason: &str,
    test_result: Option<String>,
    test_exit_code: Option<i64>,
) -> Result<MutationPipelineResult, String> {
    let updated_mutation = mutations::update_mutation_status(
        pool,
        UpdateMutationStatusInput {
            mutation_id: mutation.id.clone(),
            status: MutationStatus::Rejected,
            test_result,
            test_exit_code,
            rejection_reason: Some(reason.to_string()),
            rejected_at_step: Some(rejected_step.to_string()),
        },
    )
    .await?;

    metrics::record_audit_event(
        pool,
        "mutation_pipeline",
        "mutation_rejected",
        Some(&updated_mutation.id),
        Some(&format!(
            "{{\"step\":\"{}\",\"reason\":\"{}\"}}",
            rejected_step,
            reason.replace('"', "'")
        )),
    )
    .await?;

    let updated_task = tasks::update_task_outcome(
        pool,
        UpdateTaskOutcomeInput {
            task_id: task.id,
            status: TaskStatus::Failed,
            token_usage: None,
            context_efficiency_ratio: None,
            compliance_score: Some(0),
            checksum_before: None,
            checksum_after: None,
            error_message: Some(reason.to_string()),
        },
    )
    .await?;

    Ok(MutationPipelineResult {
        mutation: updated_mutation,
        task: updated_task,
        steps,
        shadow_dir: None,
    })
}
