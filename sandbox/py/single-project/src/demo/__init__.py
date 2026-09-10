"""Reads the version from the installed metadata, which is what pyproject.toml
declares — so nothing here repeats the number vump writes."""

from importlib.metadata import version

__version__ = version("demo")
