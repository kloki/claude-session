# claude-sessions

A Claude Code session tracker module for [waybar](https://github.com/Alexays/Waybar) that works for me.

# Install

## Binaries

Check [Releases](https://github.com/kloki/claude-sessions/releases) for binaries and installers

# Configure

## Claude hooks

Add to your Claude Code `settings.json`:

```json
{
  "hooks": {
    "SessionStart": [
      {
        "hooks": [
          { "type": "command", "command": "claude-sessions process-webhook" }
        ]
      }
    ],
    "SessionEnd": [
      {
        "hooks": [
          { "type": "command", "command": "claude-sessions process-webhook" }
        ]
      }
    ],
    "Stop": [
      {
        "hooks": [
          { "type": "command", "command": "claude-sessions process-webhook" }
        ]
      }
    ],
    "Notification": [
      {
        "hooks": [
          { "type": "command", "command": "claude-sessions process-webhook" }
        ]
      }
    ],
    "UserPromptSubmit": [
      {
        "hooks": [
          { "type": "command", "command": "claude-sessions process-webhook" }
        ]
      }
    ],
    "PermissionRequest": [
      {
        "hooks": [
          { "type": "command", "command": "claude-sessions process-webhook" }
        ]
      }
    ],
    "PreToolUse": [
      {
        "hooks": [
          { "type": "command", "command": "claude-sessions process-webhook" }
        ]
      }
    ]
  }
}
```

## Waybar

Add this to your `config.jsonc`

```json
{
  "custom/claude-sessions": {
    "exec": "~/.cargo/bin/claude-sessions waybar",
    "return-type": "json",
    "interval": 5
  }
}
```

## Styling

The module sets a CSS class based on the state of your sessions. Add to your `style.css`:

```css
#custom-claude-sessions {
  /* default styles */
}

#custom-claude-sessions.claude-idle {
  color: #888888;
}

#custom-claude-sessions.claude-active {
  color: #89b4fa; /* Claude is thinking */
}

#custom-claude-sessions.claude-waiting {
  color: #f38ba8; /* Claude is waiting for your input */
}
```

The classes are mutually exclusive and follow this priority:

| Class            | Meaning                                   |
| ---------------- | ----------------------------------------- |
| `claude-waiting` | At least one session is waiting for input |
| `claude-active`  | At least one session is actively thinking |
| `claude-idle`    | All sessions are idle                     |
