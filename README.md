# 🦀 Blogrust

**A fast, intuitive, and extensible CMS written in Rust—powered by an embedded Rhai scripting engine.**

[![License](https://shields.io)](LICENSE)
[![Built with Rust](https://shields.io)](https://rust-lang.org)
[![Powered by Rhai](https://shields.io)](https://rhai.rs)

Blogrust combines an effortless writing experience with the raw performance, memory safety, and low resource footprint of Rust. Designed for creators who want simplicity and developers who want full control without heavy legacy bloat.

🌐 **Live Demo & Docs:** [Blogrust.com](https://blogrust.com)

---

## ✨ Why Blogrust?

* **Blazingly Fast Core:** Built in Rust for minimal memory usage and instant response times.
* **Zero-Compile Plugins with Rhai:** Extend functionality, hook into lifecycle events, and build custom logic using embedded [Rhai](https://rhai.rs) scripts—no need to recompile the core binary.
* **Flexible Theme Engine:** Create clean, customizable themes effortlessly.
* **Native i18n Support:** Multi-language content and localization built right into the core.
* **Docs-as-Posts Architecture:** Out of the box, Blogrust ships with Markdown posts that double as interactive documentation. Learn how it works by exploring the live content itself.

---

## 🧩 Build Plugins & Themes (Call for Contributors!)

We are actively growing the Blogrust ecosystem and looking for early builders! Because Blogrust embeds **Rhai**, writing plugins feels familiar to Rust developers while offering safe, sandboxed, hot-reloadable execution.

### Quick Example: A Simple Rhai Plugin
Drop a `.rhai` file into your `/plugins` directory to hook into Blogrust events:

```rhai
// plugins/hello_world.rhai
fn on_post_render(post) {
    post.content += "\n\n*Rendered with Blogrust & Rhai 🦀*";
    return post;
}
```

### Ways to Get Involved:
1. **Build a Plugin:** SEO tools, RSS generators, analytics hooks, or syntax highlighters.
2. **Design a Theme:** Create minimal, developer-focused, or magazine-style themes.
3. **Share Your Work:** Open a PR to add your plugin or theme to our upcoming **Community Showcase**!

---

## 🚀 Quick Start

```bash
# Clone the repository
git clone https://github.com
cd blogrust

# Run Blogrust (includes built-in Markdown docs as starter posts)
cargo run --release
```

Once running, open your browser and visit `http://localhost:8080`. The pre-loaded Markdown posts will guide you through configuration, i18n setup, and building your first Rhai plugin.

---

## 📖 Documentation

Blogrust eats its own dogfood: **the default Markdown posts included in the repository serve as your primary documentation**. Spin up a local instance or visit [Blogrust.com](https://blogrust.com) to read the guides on:
* Architecture & Configuration
* Internationalization (`i18n`) Setup
* Rhai Plugin API & Available Hooks
* Theme Development

---

## 🤝 Contributing

Contributions, bug reports, and feature requests are welcome! Feel free to check the [issues page](https://github.com) or submit a Pull Request.

If you like the project, consider giving it a **⭐ Star** to help more Rust developers discover it!

