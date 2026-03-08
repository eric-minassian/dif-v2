//! Visual test runner for Dif.
//!
//! Follows Zed's visual testing pattern: uses real macOS Metal rendering with
//! deterministic task scheduling to capture screenshots and compare against baselines.
//!
//! Run:       cargo run --bin visual_test_runner --features visual-tests
//! Update:    UPDATE_BASELINE=1 cargo run --bin visual_test_runner --features visual-tests
//! Output:    target/visual_tests/

mod app;
mod assets;

use std::path::PathBuf;

use anyhow::{Context, Result};
use gpui::{
    AnyWindowHandle, AppContext, Size, VisualTestAppContext, WindowBounds, WindowOptions, point,
    px, size,
};
use image::RgbaImage;

use assets::Assets;

// ── Configuration ──────────────────────────────────────────────────────────

const MATCH_THRESHOLD: f64 = 0.99;
const PIXEL_TOLERANCE: u8 = 2;
const DEFAULT_WINDOW_SIZE: Size<gpui::Pixels> = size(px(1280.0), px(800.0));

fn offscreen_window_options() -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(gpui::Bounds {
            origin: point(px(-10000.0), px(-10000.0)),
            size: DEFAULT_WINDOW_SIZE,
        })),
        focus: false,
        show: true,
        ..Default::default()
    }
}

// ── Types ──────────────────────────────────────────────────────────────────

struct TestResult {
    name: String,
    outcome: TestOutcome,
}

enum TestOutcome {
    Passed,
    BaselineUpdated(PathBuf),
    Failed { match_pct: f64, diff_path: PathBuf },
    Error(String),
}

struct ImageComparison {
    match_percentage: f64,
    diff_image: RgbaImage,
}

// ── Screenshot comparison ──────────────────────────────────────────────────

fn compare_images(actual: &RgbaImage, expected: &RgbaImage) -> ImageComparison {
    let width = actual.width().max(expected.width());
    let height = actual.height().max(expected.height());
    let total_pixels = (width * height) as u64;

    let mut diff_image = RgbaImage::new(width, height);
    let mut matching_pixels: u64 = 0;

    for y in 0..height {
        for x in 0..width {
            let actual_px = if x < actual.width() && y < actual.height() {
                *actual.get_pixel(x, y)
            } else {
                image::Rgba([0, 0, 0, 0])
            };
            let expected_px = if x < expected.width() && y < expected.height() {
                *expected.get_pixel(x, y)
            } else {
                image::Rgba([0, 0, 0, 0])
            };

            let matches = actual_px
                .0
                .iter()
                .zip(expected_px.0.iter())
                .all(|(a, e)| a.abs_diff(*e) <= PIXEL_TOLERANCE);

            if matches {
                matching_pixels += 1;
                diff_image.put_pixel(x, y, image::Rgba([0, 200, 0, 128]));
            } else {
                diff_image.put_pixel(x, y, image::Rgba([255, 0, 0, 200]));
            }
        }
    }

    let match_percentage = if total_pixels == 0 {
        1.0
    } else {
        matching_pixels as f64 / total_pixels as f64
    };

    ImageComparison {
        match_percentage,
        diff_image,
    }
}

// ── Baseline management ────────────────────────────────────────────────────

fn baseline_dir() -> PathBuf {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(manifest_dir).join("test_fixtures/visual_tests")
}

fn output_dir() -> PathBuf {
    std::env::var("VISUAL_TEST_OUTPUT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let target = std::env::var("CARGO_TARGET_DIR").unwrap_or_else(|_| "target".to_string());
            PathBuf::from(target).join("visual_tests")
        })
}

fn run_visual_test(
    test_name: &str,
    window: AnyWindowHandle,
    cx: &mut VisualTestAppContext,
    update_baseline: bool,
) -> Result<TestOutcome> {
    // Ensure full render
    cx.update_window(window, |_, window, _cx| {
        window.refresh();
    })?;
    cx.run_until_parked();

    let screenshot = cx.capture_screenshot(window)?;

    let output = output_dir();
    std::fs::create_dir_all(&output)?;

    let actual_path = output.join(format!("{test_name}.png"));
    screenshot.save(&actual_path)?;

    let baselines = baseline_dir();
    let baseline_path = baselines.join(format!("{test_name}.png"));

    if update_baseline || !baseline_path.exists() {
        std::fs::create_dir_all(&baselines)?;
        screenshot.save(&baseline_path)?;
        return Ok(TestOutcome::BaselineUpdated(baseline_path));
    }

    let expected = image::open(&baseline_path)
        .with_context(|| format!("Failed to open baseline: {}", baseline_path.display()))?
        .to_rgba8();

    let comparison = compare_images(&screenshot, &expected);

    if comparison.match_percentage >= MATCH_THRESHOLD {
        Ok(TestOutcome::Passed)
    } else {
        let diff_path = output.join(format!("{test_name}_diff.png"));
        comparison.diff_image.save(&diff_path)?;
        Ok(TestOutcome::Failed {
            match_pct: comparison.match_percentage,
            diff_path,
        })
    }
}

// ── Test definitions ───────────────────────────────────────────────────────

fn test_empty_workspace(
    cx: &mut VisualTestAppContext,
    update_baseline: bool,
) -> Result<TestOutcome> {
    let window = cx.update(|cx| {
        cx.open_window(offscreen_window_options(), |window, cx| {
            cx.new(|cx| {
                workspace::WorkspaceView::new(workspace::config::AppConfig::default(), window, cx)
            })
        })
    })?;
    cx.run_until_parked();

    let result = run_visual_test("empty_workspace", window.into(), cx, update_baseline);

    cx.update_window(window.into(), |_, window, _cx| {
        window.remove_window();
    })?;

    result
}

fn test_workspace_with_config(
    cx: &mut VisualTestAppContext,
    update_baseline: bool,
) -> Result<TestOutcome> {
    let config = workspace::config::AppConfig {
        projects: vec![workspace::config::SavedProject {
            repo_root: PathBuf::from("/example/my-project"),
            display_name: "my-project".to_string(),
            last_known_valid: false,
            sessions: vec![
                workspace::config::SavedSession {
                    id: "1".to_string(),
                    name: "main".to_string(),
                    worktree_path: None,
                },
                workspace::config::SavedSession {
                    id: "2".to_string(),
                    name: "feat/login".to_string(),
                    worktree_path: None,
                },
            ],
            last_selected_session: Some("1".to_string()),
            settings: Default::default(),
        }],
        last_selected_repo: None,
        ..Default::default()
    };

    let window = cx.update(|cx| {
        cx.open_window(offscreen_window_options(), |window, cx| {
            cx.new(|cx| workspace::WorkspaceView::new(config, window, cx))
        })
    })?;
    cx.run_until_parked();

    let result = run_visual_test("workspace_with_config", window.into(), cx, update_baseline);

    cx.update_window(window.into(), |_, window, _cx| {
        window.remove_window();
    })?;

    result
}

fn test_help_view(cx: &mut VisualTestAppContext, update_baseline: bool) -> Result<TestOutcome> {
    let window = cx.update(|cx| {
        cx.open_window(offscreen_window_options(), |window, cx| {
            cx.new(|cx| {
                workspace::WorkspaceView::new(workspace::config::AppConfig::default(), window, cx)
            })
        })
    })?;
    cx.run_until_parked();

    // Toggle help via action dispatch
    cx.dispatch_action(window.into(), workspace::ToggleHelp);
    cx.run_until_parked();

    let result = run_visual_test("help_view", window.into(), cx, update_baseline);

    cx.update_window(window.into(), |_, window, _cx| {
        window.remove_window();
    })?;

    result
}

// ── Main ───────────────────────────────────────────────────────────────────

fn main() {
    #[cfg(not(target_os = "macos"))]
    {
        eprintln!("Visual tests require macOS (Metal rendering).");
        std::process::exit(1);
    }

    #[cfg(target_os = "macos")]
    {
        let platform = gpui_platform::current_platform(false);
        let mut cx = VisualTestAppContext::with_asset_source(platform, std::sync::Arc::new(Assets));

        // Register keybindings so action dispatch works
        cx.update(|cx| {
            cx.bind_keys(ui::text_input::key_bindings());
            let entries = workspace::keybindings::default_keybindings();
            cx.bind_keys(workspace::keybindings::to_gpui_keybindings(&entries));
        });

        let update_baseline = std::env::var("UPDATE_BASELINE").is_ok();

        let tests: Vec<(
            &str,
            Box<dyn FnOnce(&mut VisualTestAppContext, bool) -> Result<TestOutcome>>,
        )> = vec![
            ("empty_workspace", Box::new(test_empty_workspace)),
            (
                "workspace_with_config",
                Box::new(test_workspace_with_config),
            ),
            ("help_view", Box::new(test_help_view)),
        ];

        let mut results = Vec::new();
        for (name, test_fn) in tests {
            let outcome = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                test_fn(&mut cx, update_baseline)
            })) {
                Ok(Ok(outcome)) => outcome,
                Ok(Err(e)) => TestOutcome::Error(format!("{e:#}")),
                Err(_) => TestOutcome::Error("panic".to_string()),
            };
            results.push(TestResult {
                name: name.to_string(),
                outcome,
            });
        }

        // Print summary
        println!("\n=== Visual Test Results ===\n");
        let mut passed = 0;
        let mut failed = 0;
        let mut updated = 0;
        let mut errors = 0;

        for result in &results {
            match &result.outcome {
                TestOutcome::Passed => {
                    println!("  PASS  {}", result.name);
                    passed += 1;
                }
                TestOutcome::BaselineUpdated(path) => {
                    println!("  NEW   {}  -> {}", result.name, path.display());
                    updated += 1;
                }
                TestOutcome::Failed {
                    match_pct,
                    diff_path,
                } => {
                    println!(
                        "  FAIL  {}  ({:.1}% match, diff: {})",
                        result.name,
                        match_pct * 100.0,
                        diff_path.display()
                    );
                    failed += 1;
                }
                TestOutcome::Error(e) => {
                    println!("  ERR   {}  ({})", result.name, e);
                    errors += 1;
                }
            }
        }

        println!();
        println!(
            "  {} passed, {} failed, {} updated, {} errors",
            passed, failed, updated, errors
        );

        if failed > 0 || errors > 0 {
            std::process::exit(1);
        }
    }
}
