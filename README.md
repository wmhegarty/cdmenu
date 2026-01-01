# cdMenu

A lightweight macOS and Windows menubar/system tray application for monitoring Bitbucket Cloud pipeline statuses.

## Features

- **System Tray Status** - Green, red, or gray icon showing overall pipeline health at a glance
- **Pipeline Details** - Click the tray icon to see all monitored pipelines grouped by project
- **Quick Navigation** - Click any pipeline to open it directly in your browser
- **Desktop Notifications** - Get notified when pipelines fail or recover
- **Paused Pipeline Detection** - See which pipelines are waiting for manual approval with step names
- **Configurable Polling** - Set your preferred check interval (default: 60 seconds)
- **Multi-Platform** - Native builds for macOS (Apple Silicon & Intel) and Windows

## Installation

### Download

Get the latest release from the [Releases page](https://github.com/wmhegarty/cdmenu/releases):

| Platform | Download |
|----------|----------|
| macOS (Apple Silicon) | `cdMenu_x.x.x_aarch64.dmg` |
| macOS (Intel) | `cdMenu_x.x.x_x64.dmg` |
| Windows | `cdMenu_x.x.x_x64-setup.exe` |

### macOS Note

The app is not yet code-signed. After installing, you may need to remove the quarantine attribute:

```bash
xattr -cr /Applications/cdMenu.app
```

## Setup

### 1. Create a Bitbucket App Password

1. Go to [Bitbucket App Passwords](https://bitbucket.org/account/settings/app-passwords/)
2. Click **Create app password**
3. Give it a name (e.g., "cdMenu")
4. Select permissions:
   - **Repositories: Read**
   - **Pipelines: Read**
5. Copy the generated password

### 2. Configure cdMenu

1. Click the cdMenu tray icon and select **Settings...**
2. Enter your Bitbucket email and the app password you created
3. Click **Save Credentials**
4. Select your workspace, project, and repositories to monitor
5. Click **Add** for each repository you want to track

## Usage

- **Green icon** - All pipelines are healthy
- **Red icon** - One or more pipelines have failed
- **Gray icon** - Loading, not configured, or no pipelines monitored

Click the tray icon to:
- View all monitored pipelines with their current status
- Click a pipeline to open it in your browser
- Refresh status manually
- Access settings

## Building from Source

### Prerequisites

- [Node.js](https://nodejs.org/) (v18+)
- [Rust](https://rustup.rs/) (latest stable)
- [Tauri CLI](https://tauri.app/) v2

### Build

```bash
# Clone the repository
git clone https://github.com/wmhegarty/cdmenu.git
cd cdmenu

# Install dependencies
npm install

# Run in development mode
cargo tauri dev

# Build for production
cargo tauri build
```

The built application will be in `src-tauri/target/release/bundle/`.

## Tech Stack

- **Framework**: [Tauri v2](https://tauri.app/) (Rust + Web)
- **Backend**: Rust
- **Frontend**: Vanilla HTML/CSS/JavaScript
- **API**: Bitbucket Cloud REST API

## License

MIT

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.
