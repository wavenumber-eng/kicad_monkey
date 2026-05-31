# Design Documentation

This folder records durable command, interface, data-flow, and format design
notes.

The master HTML entry point is `index.html`. Command design docs live under
`cli/`, API/interface design docs live under `api/`, and all public design HTML
uses `styles.css`.

L99 signoff enforces:

- every command in `docs/contracts/command_manifest.v0.json` has
  `docs/design/cli/<command>.html`;
- every command doc declares usage, arguments, output, tests, and config
  contract status;
- every public dataclass and every listed major interface has a design-doc
  section with rationale, purpose, test requirements, working definition, and
  Rack test ownership.

