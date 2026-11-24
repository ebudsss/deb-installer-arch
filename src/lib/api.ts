import { invoke } from "@tauri-apps/api/core";

export type Risk = "Safe" | "Caution" | "Dangerous";
export type DepStatus = "Installed" | "Available" | "AurOnly" | "Missing" | "Ignored";
export type Confidence = "High" | "Medium" | "Low";
export type ResolutionSource = "Mapping" | "FileSearch" | "NameMatch" | "Ignored" | "Unresolved";

export interface EntryView {
  path: string;
  original: string;
  kind: "file" | "dir" | "symlink" | "hardlink";
  size: number;
  link_target: string | null;
}

export interface SensitivePath {
  path: string;
  reason: string;
  level: "Forbidden" | "Warning";
}

export interface DepResolution {
  debian: string;
  constraint: string | null;
  alternatives: string[];
  arch_name: string | null;
  status: DepStatus;
  confidence: Confidence;
  source: ResolutionSource;
  note: string | null;
}

export interface DepSummary {
  total: number;
  installed: number;
  available: number;
  aur: number;
  missing: number;
  ignored: number;
}

export interface Finding {
  script: string;
  line: number;
  text: string;
  pattern: string;
  risk: Risk;
  advice: string;
}

export interface TranslatedCommand {
  source: string;
  replacement: string;
  hook: string;
}

export interface ScriptReport {
  findings: Finding[];
  translated: TranslatedCommand[];
}

export interface Alternative {
  name: string;
  source: "Repo" | "Aur";
  version: string | null;
  description: string | null;
}

export type ArchiveLayout = { kind: "system" } | { kind: "portable"; root: string | null };

export interface Analysis {
  path: string;
  /** `deb` untuk paket Debian, `archive` untuk arsip tar. */
  kind: "deb" | "archive";
  /** Tata letak arsip; null untuk `.deb`. */
  layout: ArchiveLayout | null;
  /** Binari yang ditautkan ke /usr/bin (arsip portable). */
  executables: string[];
  package: string;
  version: string;
  arch: string;
  arch_mapped: string;
  suggested_pkgname: string;
  description: string;
  maintainer: string;
  homepage: string;
  file_size: number;
  installed_size: number;
  sha256: string;
  data_member: string;
  file_count: number;
  entries: EntryView[];
  entries_truncated: boolean;
  sensitive: SensitivePath[];
  deps: DepResolution[];
  dep_summary: DepSummary;
  scripts: ScriptReport;
  install_script: string | null;
  max_risk: Risk;
  alternatives: Alternative[];
  blocked: boolean;
}

export interface BuildRequest {
  path: string;
  output_dir?: string | null;
  include_install_script: boolean;
  /** Level zstd 1-22; 10 adalah kompromi waktu/ukuran yang dipakai UI. */
  compression_level: number;
  keep_staging: boolean;
}

export interface BuildResult {
  package_path: string;
  pkgname: string;
  pkgver: string;
  pkgrel: string;
  arch: string;
  installed_size: number;
  file_count: number;
  warnings: SensitivePath[];
  notes: string[];
  install_script: string | null;
  dependencies: string[];
}

export interface InstallOutcome {
  success: boolean;
  exit_code: number | null;
  log: string[];
  diagnosis: string | null;
  diagnosis_message: string | null;
}

export interface InstallRecord {
  id: string;
  deb_name: string;
  deb_version: string;
  deb_path: string;
  deb_sha256: string;
  pkgname: string;
  pkgver: string;
  package_path: string;
  converted_at: number;
  installed_at: number | null;
  uninstalled_at: number | null;
}

export interface Settings {
  language: "id" | "en";
  theme: "system" | "light" | "dark";
  output_dir: string | null;
  keep_packages: boolean;
  check_aur: boolean;
  custom_mapping: string;
}

export interface Progress {
  stage: string;
  message: string;
}

export const api = {
  analyze: (path: string) => invoke<Analysis>("analyze", { path }),
  build: (request: BuildRequest) => invoke<BuildResult>("build_package", { request }),
  install: (packagePath: string, keepPackage = true) =>
    invoke<InstallOutcome>("install_package", { packagePath, keepPackage }),
  uninstall: (pkgname: string) =>
    invoke<InstallOutcome>("uninstall_package", { pkgname }),
  listInstalled: () => invoke<InstallRecord[]>("list_installed"),
  getSettings: () => invoke<Settings>("get_settings"),
  saveSettings: (settings: Settings) => invoke<void>("save_settings", { settings }),
  refreshFileDb: () => invoke<boolean>("refresh_file_db"),
  deleteRecord: (pkgname: string) => invoke<void>("delete_record", { pkgname }),
};

export function formatBytes(bytes: number): string {
  const units = ["B", "KiB", "MiB", "GiB"];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return unit === 0 ? `${bytes} B` : `${value.toFixed(1)} ${units[unit]}`;
}
