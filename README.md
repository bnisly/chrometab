# ChromeTab (Rust)

A blazingly fast command-line tool for managing Google Chrome tabs using the Chrome DevTools Protocol. Written in Rust for maximum performance and reliability.

[![Crates.io](https://img.shields.io/crates/v/chrometab.svg)](https://crates.io/crates/chrometab)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.70+-orange.svg)](https://www.rust-lang.org/)

## Features

- 🦀 **Written in Rust** - Fast, safe, and reliable
- 🔍 **Search tabs** by title or URL with wildcard support
- 🎯 **Activate tabs** instantly from command line
- 📋 **List all tabs** with interactive selection
- 🔄 **Find duplicate tabs** by URL
- ✖️ **Close tabs** individually or in bulk
- 🆕 **Open new tabs** with specified URLs
- 🌐 **WebSocket server/client mode** for remote control (coming soon)
- 💻 **Cross-platform** support (Windows, Linux, macOS)
- 🎨 **Colorful terminal output** for better readability
- ⚡ **Zero dependencies** on browser extensions

## Installation

### Using Cargo

```bash
cargo install chrometab
```

### From Source

```bash
git clone https://github.com/cumulus13/chrometab-rust.git
cd chrometab-rust
cargo build --release
```

The binary will be available at `target/release/chrometab`

### Download Pre-built Binaries

Download from [Releases](https://github.com/cumulus13/chrometab-rust/releases) page.

## Prerequisites

### 1. Enable Chrome Remote Debugging

Launch Chrome with remote debugging enabled:

#### Windows
```cmd
chrome.exe --remote-debugging-port=9222
```

#### Linux
```bash
google-chrome --remote-debugging-port=9222
```

#### macOS
```bash
/Applications/Google\ Chrome.app/Contents/MacOS/Google\ Chrome --remote-debugging-port=9222
```

### 2. Environment Variables (Optional)

```bash
# Set custom Chrome debugging host and port
export CHROME_REMOTE_DEBUGGING_HOST=localhost
export CHROME_REMOTE_DEBUGGING_PORT=9222
```

### 3. Platform-Specific Requirements

**Linux**: Install `xdotool` for window activation
```bash
sudo apt-get install xdotool  # Ubuntu/Debian
sudo dnf install xdotool      # Fedora
sudo pacman -S xdotool        # Arch
```

**Windows**: No additional requirements

**macOS**: Grant Terminal accessibility permissions in System Preferences

### Using with Brave

ChromeTab works with **Brave** (and any Chromium-based browser) the same way as Chrome. Brave uses the same Chrome DevTools Protocol.

1. **Launch Brave with remote debugging:**
   - **macOS**: `/Applications/Brave\ Browser.app/Contents/MacOS/Brave\ Browser --remote-debugging-port=9222`
   - **Linux**: `brave-browser --remote-debugging-port=9222` (or `brave --remote-debugging-port=9222` depending on your install)
   - **Windows**: `brave.exe --remote-debugging-port=9222`

2. Run `chrometab` as usual. With the default `--browser auto`, the tool detects Brave from the CDP version response and brings the Brave window to front when you activate a tab.

3. To force Brave (e.g. if auto-detection fails), use: `chrometab --browser brave --list`

You can also set the `CHROMETAB_BROWSER` environment variable to `chrome`, `brave`, or `auto`.

## Usage

### Basic Commands

```bash
# List all tabs interactively
chrometab --list

# Search for tabs matching pattern
chrometab "youtube"

# Search with wildcard
chrometab "*github*"

# Open a new tab
chrometab --url "https://google.com"

# Show URLs in listing
chrometab --list --show-url

# Find duplicate tabs
chrometab --find-duplicate
```

### Interactive Mode

When running `chrometab --list` or when multiple matches are found, you enter interactive mode:

**Commands:**
- **Number** - Activate the selected tab
- **s** - Toggle URL display
- **r** - Refresh tab list
- **u** - Open new tab with URL
- **fd** - Find duplicate tabs
- **[n]c** - Close tab number n (e.g., `3c`)
- **[n1,n2]c** - Close multiple tabs (e.g., `1,3,5c`)
- **[n1-nx]c** - Close range of tabs (e.g., `1-5c`)
- **q/x** - Quit

### Command Line Options

```
USAGE:
    chrometab [OPTIONS] [PATTERN]

ARGUMENTS:
    [PATTERN]    Pattern to match tab titles or URLs

OPTIONS:
    -l, --list              List all available tabs
    -a, --active-only       Show only active tabs (exclude chrome-extension://)
    -s, --show-url          Display URLs alongside tab titles
    --find-duplicate        Find and list tabs with duplicate URLs
    -u, --url <URL>         Open a new tab with the specified URL
    -f, --force <PLATFORM>  Force platform for window activation (windows/linux/darwin)
    --browser <chrome|brave|auto>  Browser for window activation [default: auto] [env: CHROMETAB_BROWSER]
    --host <HOST>           Chrome remote debugging host [env: CHROME_REMOTE_DEBUGGING_HOST]
    --port <PORT>           Chrome remote debugging port [env: CHROME_REMOTE_DEBUGGING_PORT]
    --debug                 Enable debug output
    -h, --help              Print help
    -V, --version           Print version

SUBCOMMANDS:
    serve     Run as WebSocket server for remote control (coming soon)
    client    Send pattern to WebSocket server (coming soon)
```

## Examples

### Basic Tab Management

```bash
# Find and activate a tab with "github" in title or URL
chrometab github

# List all tabs with URLs visible
chrometab --list --show-url

# Find tabs with duplicate URLs
chrometab --find-duplicate

# Open multiple tabs
chrometab --url "https://github.com"
chrometab --url "https://google.com"
```

### Advanced Search

```bash
# Search with wildcard at the beginning
chrometab "*react*"

# Search for YouTube videos
chrometab "youtube*"

# Search for specific domain
chrometab "*github.com*"

# Case-insensitive search (automatic)
chrometab "YOUTUBE"  # matches "YouTube", "youtube", etc.
```

### Bulk Operations

In interactive mode, close multiple tabs:
```bash
chrometab --list

# Then type:
1,3,5c      # Close tabs 1, 3, and 5
1-5c        # Close tabs 1 through 5
2,5-8,10c   # Close tabs 2, 5 through 8, and 10
```

### Custom Chrome Instance

```bash
# Connect to Chrome on different host/port
chrometab --host 192.168.1.100 --port 9223 --list

# Or use environment variables
export CHROME_REMOTE_DEBUGGING_HOST=192.168.1.100
export CHROME_REMOTE_DEBUGGING_PORT=9223
chrometab --list
```

## Profile Name Display

**Note**: Chrome DevTools Protocol does not provide direct access to profile names. However, you can identify profiles through:

1. **Browser Context ID** - Available in the CDP response (for isolated contexts)
2. **User Data Directory** - The Chrome launch parameter `--user-data-dir`
3. **Tab Title Patterns** - Profiles sometimes show in window titles

To work with specific profiles:

```bash
# Launch Chrome with specific profile
chrome.exe --remote-debugging-port=9222 --profile-directory="Profile 1"

# Or use user-data-dir
chrome.exe --remote-debugging-port=9222 --user-data-dir="C:\Users\YourName\ChromeProfiles\Work"
```

The tool will interact with whichever Chrome instance is running on the debugging port.

## How It Works

ChromeTab communicates with Chrome via the **Chrome DevTools Protocol (CDP)**:

1. Chrome launches with `--remote-debugging-port=9222`
2. ChromeTab connects to `http://localhost:9222/json/version`
3. Retrieves WebSocket URL for CDP communication
4. Sends CDP commands:
   - `Target.getTargets` - List all tabs
   - `Target.activateTarget` - Activate a tab
   - `Target.closeTarget` - Close a tab
   - `Target.createTarget` - Open new tab
5. Platform-specific window activation:
   - **Windows**: Win32 API (`EnumWindows`, `SetForegroundWindow`)
   - **Linux**: `xdotool` command
   - **macOS**: AppleScript

## Architecture

```
┌─────────────────┐
│   ChromeTab CLI │
└────────┬────────┘
         │
         ├──── Chrome DevTools Protocol (WebSocket)
         │     └─── Target Domain
         │          ├─── getTargets
         │          ├─── activateTarget
         │          ├─── closeTarget
         │          └─── createTarget
         │
         └──── Window Management
               ├─── Windows: Win32 API
               ├─── Linux: xdotool
               └─── macOS: AppleScript
```

## Performance

Rust version benefits:
- **Startup time**: ~10ms (vs ~100ms Python)
- **Memory usage**: ~2MB (vs ~30MB Python)
- **Binary size**: ~3MB (vs ~50MB Python with dependencies)
- **Tab switching**: <50ms response time
- **Zero runtime dependencies**

## Building for Different Platforms

### Cross-compilation

```bash
# Windows from Linux
cargo build --release --target x86_64-pc-windows-gnu

# Linux from macOS
cargo build --release --target x86_64-unknown-linux-gnu

# macOS from Linux (requires osxcross)
cargo build --release --target x86_64-apple-darwin
```

### Optimized Release Build

```bash
cargo build --release

# With additional optimizations
RUSTFLAGS="-C target-cpu=native" cargo build --release
```

## Troubleshooting

### Chrome Not Found

**Error**: "Failed to connect to Chrome"

**Solution**: 
1. Ensure Chrome is running with `--remote-debugging-port=9222`
2. Check if port 9222 is accessible: `curl http://localhost:9222/json/version`
3. Check firewall settings

### Window Not Brought to Front

**Windows**:
- Run as Administrator if needed
- Check if Chrome is minimized to system tray

**Linux**:
- Install xdotool: `sudo apt-get install xdotool`
- Check if DISPLAY variable is set

**macOS**:
- Grant Terminal accessibility permissions
- System Preferences → Security & Privacy → Privacy → Accessibility

### WebSocket Connection Failed

**Error**: "Failed to connect to WebSocket"

**Solution**:
1. Verify CDP URL: `curl http://localhost:9222/json/version`
2. Check if another tool is using the debugging connection
3. Restart Chrome with debugging enabled

### Permission Denied (Linux/macOS)

```bash
# Make binary executable
chmod +x chrometab

# Or install via cargo
cargo install chrometab
```

## Development

### Running Tests

```bash
cargo test
```

### Running with Debug Output

```bash
cargo run -- --debug --list
```

### Code Style

```bash
# Format code
cargo fmt

# Run clippy
cargo clippy

# Check for common issues
cargo check
```

## Dependencies

- **tokio** - Async runtime
- **tokio-tungstenite** - WebSocket client
- **serde** - JSON serialization
- **reqwest** - HTTP client
- **clap** - CLI argument parsing
- **colored** - Terminal colors
- **regex** - Pattern matching
- **anyhow** - Error handling

### Windows-specific
- **winapi** - Win32 API bindings

## Roadmap

- [x] Basic tab management (list, activate, close)
- [x] Pattern matching with wildcards
- [x] Find duplicate tabs
- [x] Interactive mode
- [x] Cross-platform window activation
- [ ] WebSocket server/client mode
- [ ] Tab grouping support
- [ ] Profile name detection (from Preferences file)
- [ ] Browser context isolation
- [ ] Session management
- [ ] Configuration file support
- [ ] Bash/Zsh completion scripts

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

### Development Process

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add some amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

### Code Guidelines

- Follow Rust idioms and best practices
- Add tests for new features
- Update documentation
- Run `cargo fmt` and `cargo clippy` before committing

## Publishing to crates.io

```bash
# Update version in Cargo.toml
# Then publish
cargo publish
```

Users can then install with:
```bash
cargo install chrometab
```

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Author

**Hadi Cahyadi** - [cumulus13@gmail.com](mailto:cumulus13@gmail.com)

[![Buy Me a Coffee](https://www.buymeacoffee.com/assets/img/custom_images/orange_img.png)](https://www.buymeacoffee.com/cumulus13)

[![Donate via Ko-fi](https://ko-fi.com/img/githubbutton_sm.svg)](https://ko-fi.com/cumulus13)

[Support me on Patreon](https://www.patreon.com/cumulus13)

## Acknowledgments

- Inspired by the Python version [chrometab](https://github.com/cumulus13/chrometab-py)
- Go version [chrometab-go](https://github.com/cumulus13/chrometab-go)
- Built with [Chrome DevTools Protocol](https://chromedevtools.github.io/devtools-protocol/)
- Uses [tokio](https://tokio.rs/) async runtime
- Colors by [colored](https://github.com/mackwic/colored)

## Related Projects

- [chrometab (Python)](https://github.com/cumulus13/chrometab-py) - Original Python version
- [chrometab-go](https://github.com/cumulus13/chrometab-go) - Go version

## Comparison with Other Versions

| Feature | Python | Go | **Rust** |
|---------|--------|-----|----------|
| Startup Time | ~100ms | ~20ms | **~10ms** |
| Memory Usage | ~30MB | ~5MB | **~2MB** |
| Binary Size | ~50MB | ~8MB | **~3MB** |
| Runtime | Required | None | **None** |
| Dependencies | Many | Few | **Minimal** |
| Performance | Good | Great | **Excellent** |
| Safety | Dynamic | Static | **Memory-Safe** |

## FAQ

**Q: Do I need to install Chrome extensions?**  
A: No, ChromeTab works directly with Chrome DevTools Protocol.

**Q: Can I use this with Chromium?**  
A: Yes, Chromium also supports Chrome DevTools Protocol.

**Q: Does this work with Chrome profiles?**  
A: Yes, connect to the specific Chrome instance running with your desired profile.

**Q: Can I automate tab management?**  
A: Yes, use the command-line interface in scripts.

**Q: Is it safe to use?**  
A: Yes, it only uses the official Chrome DevTools Protocol API.

**Q: Why Rust instead of Python/Go?**  
A: Rust provides the best combination of performance, memory safety, and zero-cost abstractions.

---

**⭐ Star this repository if you find it useful!**

**📦 Install**: `cargo install chrometab`