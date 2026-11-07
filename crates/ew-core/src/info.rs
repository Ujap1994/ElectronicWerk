//! Static information about the running engine.

use serde::Serialize;

/// Describes the engine build and the machine it runs on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineInfo {
    /// Human-readable engine name.
    pub name: &'static str,
    /// Engine version (`MAJOR.MINOR.PATCH`).
    pub version: &'static str,
    /// Operating system the engine was compiled for (for example `macos`).
    pub os: &'static str,
    /// CPU architecture the engine was compiled for (for example `aarch64`).
    pub arch: &'static str,
    /// Number of worker threads the engine may use.
    pub threads: usize,
    /// `true` for debug builds.
    pub debug: bool,
}

impl EngineInfo {
    /// Collects information about the current engine build.
    #[must_use]
    pub fn current() -> Self {
        Self {
            name: "ElectronicWerk Engine",
            version: crate::VERSION,
            os: std::env::consts::OS,
            arch: std::env::consts::ARCH,
            threads: std::thread::available_parallelism().map_or(1, std::num::NonZero::get),
            debug: cfg!(debug_assertions),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_reports_workspace_version_and_target() {
        let info = EngineInfo::current();
        assert_eq!(info.version, crate::VERSION);
        assert_eq!(info.os, std::env::consts::OS);
        assert_eq!(info.arch, std::env::consts::ARCH);
        assert!(info.threads >= 1);
    }

    #[test]
    fn serializes_in_camel_case() {
        let json = serde_json::to_value(EngineInfo::current()).unwrap();
        assert!(json.get("version").is_some());
        assert!(json.get("threads").is_some());
        assert_eq!(json["name"], "ElectronicWerk Engine");
    }
}
