import type { Lang } from '../lib/highlight.ts';

export interface TrackedFile {
  /** The filename vump recognizes. */
  name: string;
  ecosystem: string;
  /** Where in the file the project's own version lives. */
  where: string;
  /** What is deliberately never mistaken for it. */
  guard: string;
  lang: Lang;
  code: string;
  /** 1-based lines vump writes. */
  emphasis: number[];
}

export const TRACKED_FILES: TrackedFile[] = [
  {
    name: 'package.json',
    ecosystem: 'npm',
    where: 'top-level version',
    guard: 'A version nested under dependencies is never mistaken for the project’s own.',
    lang: 'json',
    emphasis: [3],
    code: `{
  "name": "@acme/widget",
  "version": "1.4.0",
  "dependencies": {
    "react": "^19.0.0"
  }
}`,
  },
  {
    name: 'package-lock.json',
    ecosystem: 'npm',
    where: 'top-level version, and the root packages entry',
    guard: 'Both move together, because npm ci rejects a tree where they disagree.',
    lang: 'json',
    emphasis: [3, 7],
    code: `{
  "name": "@acme/widget",
  "version": "1.4.0",
  "lockfileVersion": 3,
  "packages": {
    "": {
      "version": "1.4.0"
    }
  }
}`,
  },
  {
    name: 'Cargo.toml',
    ecosystem: 'Cargo',
    where: '[package].version',
    guard: 'Editing the document structurally cannot reach a dependency’s own pin.',
    lang: 'toml',
    emphasis: [3],
    code: `[package]
name = "widget"
version = "1.4.0"

[dependencies]
serde = { version = "1.0", features = ["derive"] }`,
  },
  {
    name: 'Cargo.lock',
    ecosystem: 'Cargo',
    where: 'the [[package]] entry for this crate',
    guard: 'Matched by the package name the manifest declares — a locked dependency is left alone.',
    lang: 'toml',
    emphasis: [3],
    code: `[[package]]
name = "widget"
version = "1.4.0"

[[package]]
name = "serde"
version = "1.0.229"`,
  },
  {
    name: 'pyproject.toml',
    ecosystem: 'Python',
    where: '[project].version',
    guard: 'Requirements pinned under dependencies are requirements, not this project’s version.',
    lang: 'toml',
    emphasis: [3],
    code: `[project]
name = "widget"
version = "1.4.0"
dependencies = ["httpx>=0.28"]`,
  },
  {
    name: 'uv.lock',
    ecosystem: 'Python',
    where: 'the [[package]] entry for this project',
    guard: 'Written in the same run as pyproject.toml, so the pair is never half-bumped.',
    lang: 'toml',
    emphasis: [3],
    code: `[[package]]
name = "widget"
version = "1.4.0"
source = { editable = "." }`,
  },
  {
    name: '*.csproj',
    ecosystem: '.NET',
    where: '<Version> in a <PropertyGroup>',
    guard:
      'Not <AssemblyVersion>, which has four numeric parts and no pre-release. Not a <PackageReference Version>.',
    lang: 'xml',
    emphasis: [3],
    code: `<Project Sdk="Microsoft.NET.Sdk">
  <PropertyGroup>
    <Version>1.4.0</Version>
    <AssemblyVersion>1.4.0.0</AssemblyVersion>
  </PropertyGroup>
  <ItemGroup>
    <PackageReference Include="Serilog" Version="4.2.0" />
  </ItemGroup>
</Project>`,
  },
  {
    name: 'Directory.Build.props',
    ecosystem: '.NET',
    where: '<Version> in a <PropertyGroup>',
    guard:
      'Where projects sharing one version declare it. MSBuild stops at the nearest one rather than merging several.',
    lang: 'xml',
    emphasis: [3],
    code: `<Project>
  <PropertyGroup>
    <Version>1.4.0</Version>
  </PropertyGroup>
</Project>`,
  },
  {
    name: 'VERSION',
    ecosystem: 'any',
    where: 'the whole file',
    guard: 'The escape hatch: any ecosystem, any build system, one line.',
    lang: 'toml',
    emphasis: [1],
    code: `1.4.0`,
  },
];
