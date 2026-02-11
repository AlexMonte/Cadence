# Runtime JS Assets

This folder contains JavaScript assets used by the embedded webview runtime host.

## Install

```bash
cd runtime/js
npm install
```

This installs `@strudel/web`, which is loaded by:

- `runtime/web/index.html`
- `runtime/web/host.js`

The Rust runtime host process launches a webview and loads the page above in a browser context.
