import type { Lang } from '../lib/highlight.ts';

export interface TrackedFile {
  /** The filename vump recognizes. */
  name: string;
  ecosystem: string;
  /** Where in the file the project's own version lives. */
  where: string;
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
    lang: 'toml',
    emphasis: [1],
    code: `1.4.0`,
  },
];
