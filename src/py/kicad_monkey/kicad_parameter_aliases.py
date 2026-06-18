"""Generic parameter alias normalization helpers."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Mapping, cast


DEFAULT_PART_PARAMETER_ALIASES: dict[str, tuple[str, ...]] = {
    "mpn": (
        "manufacturer part number",
        "manufacturer_part_number",
        "manufacturer part",
        "manufacturer_part",
        "manufacturer pn",
        "manufacturer_pn",
        "manufacturer p/n",
        "mfr part number",
        "mfr_part_number",
        "mfr pn",
        "mfr_pn",
        "mfg part number",
        "mfg_part_number",
        "mfg pn",
        "mfg_pn",
        "manf_pn",
        "part number",
        "part_number",
        "part no",
        "part_no",
        "part #",
        "pn",
    ),
    "mfg": (
        "manufacturer",
        "manufacturer name",
        "manufacturer_name",
        "mfr",
        "mfr name",
        "mfr_name",
        "manf",
        "manf name",
        "manf_name",
    ),
    "value": (
        "comment",
    ),
    "description": (
        "desc",
        "part description",
        "part_description",
    ),
    "cad-reference": (
        "cad reference",
        "cad_reference",
    ),
}


@dataclass(frozen=True, slots=True)
class ParameterAliasResolution:
    """Resolved canonical parameters and their source raw-field names."""

    canonical_fields: dict[str, str]
    field_sources: dict[str, str]


@dataclass(frozen=True, slots=True)
class ParameterAliasConfig:
    """Canonical parameter names with ordered source-field aliases."""

    canonical_fields: Mapping[str, object]

    @classmethod
    def from_mapping(cls, mapping: Mapping[str, object]) -> ParameterAliasConfig:
        """Build an alias config from a JSON-style mapping."""
        return cls(mapping)

    def __post_init__(self) -> None:
        normalized: dict[str, tuple[str, ...]] = {}
        for canonical_name, aliases in self.canonical_fields.items():
            canonical = normalize_parameter_token(canonical_name)
            if not canonical:
                continue
            normalized[canonical] = _unique_aliases(canonical, _string_tuple(aliases))
        object.__setattr__(self, "canonical_fields", normalized)

    def aliases_for(self, canonical_name: str) -> tuple[str, ...]:
        """Return lookup aliases for a canonical parameter, including itself."""
        canonical = normalize_parameter_token(canonical_name)
        aliases = self.canonical_fields.get(canonical)
        if aliases is None:
            return (canonical,)
        return cast(tuple[str, ...], aliases)


def normalize_parameter_token(value: object) -> str:
    """Normalize a parameter name token for case-insensitive alias lookup."""
    return str(value or "").strip().casefold()


def default_part_parameter_alias_config() -> ParameterAliasConfig:
    """Return the default reusable part-parameter alias configuration."""
    return ParameterAliasConfig(DEFAULT_PART_PARAMETER_ALIASES)


def resolve_parameter_aliases(
    fields: Mapping[str, object],
    aliases: ParameterAliasConfig,
) -> ParameterAliasResolution:
    """Resolve canonical parameters from raw fields using ordered aliases."""
    lookup = _parameter_lookup(fields)
    canonical_fields: dict[str, str] = {}
    field_sources: dict[str, str] = {}
    for canonical_name in aliases.canonical_fields:
        for alias in aliases.aliases_for(canonical_name):
            found = lookup.get(normalize_parameter_token(alias))
            if found is None:
                continue
            source_name, value = found
            canonical_fields[canonical_name] = value
            field_sources[canonical_name] = source_name
            break
    return ParameterAliasResolution(
        canonical_fields=canonical_fields,
        field_sources=field_sources,
    )


def canonicalize_part_parameters(fields: Mapping[str, object]) -> dict[str, str]:
    """Return default canonical part parameters for a raw field map."""
    return resolve_parameter_aliases(
        fields,
        default_part_parameter_alias_config(),
    ).canonical_fields


def _parameter_lookup(fields: Mapping[str, object]) -> dict[str, tuple[str, str]]:
    lookup: dict[str, tuple[str, str]] = {}
    for source_name, raw_value in fields.items():
        name = str(source_name or "").strip()
        value = "" if raw_value is None else str(raw_value).strip()
        token = normalize_parameter_token(name)
        if token and value and token not in lookup:
            lookup[token] = (name, value)
    return lookup


def _string_tuple(value: object) -> tuple[str, ...]:
    if value is None:
        return ()
    if isinstance(value, str):
        return (value,)
    try:
        return tuple(str(item) for item in value)  # type: ignore[operator]
    except TypeError:
        return (str(value),)


def _unique_aliases(canonical: str, aliases: tuple[str, ...]) -> tuple[str, ...]:
    out: list[str] = []
    seen: set[str] = set()
    for alias in (canonical, *aliases):
        token = normalize_parameter_token(alias)
        if not token or token in seen:
            continue
        out.append(str(alias))
        seen.add(token)
    return tuple(out)


__all__ = [
    "DEFAULT_PART_PARAMETER_ALIASES",
    "ParameterAliasConfig",
    "ParameterAliasResolution",
    "canonicalize_part_parameters",
    "default_part_parameter_alias_config",
    "normalize_parameter_token",
    "resolve_parameter_aliases",
]
