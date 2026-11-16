# Code signing policy

> **Status:** An application to the [SignPath Foundation](https://signpath.org) free open-source code signing program is pending.
> After approval, Windows releases will carry this attribution:
> *Free code signing provided by [SignPath.io](https://about.signpath.io), certificate by [SignPath Foundation](https://signpath.org).*

## What gets signed
- Only binaries built from this repository's source by the [Release workflow](.github/workflows/release.yml), which runs on GitHub-hosted runners for each `vX.Y.Z` tag.
- The signed artifact is the Windows installer `TranslateBubble_<version>_x64-setup.exe`.
- No third-party or upstream binaries are signed.
- The product name (`Translate Bubble`) and version are set in [`src-tauri/tauri.conf.json`](src-tauri/tauri.conf.json). The workflow fails if the tag doesn't match that version.

## Team roles
| Role | Members |
|---|---|
| Authors (committers) | [@kasifraza](https://github.com/kasifraza) |
| Reviewers | [@kasifraza](https://github.com/kasifraza) |
| Approvers (approve each signing request) | [@kasifraza](https://github.com/kasifraza) |

All team members use multi-factor authentication on GitHub and SignPath. Changes from people outside the team are accepted only through reviewed pull requests. Every release must be manually approved in SignPath before it is signed.

## Privacy policy
This program will not transfer any information to other networked systems unless the user specifically requests it:

- **Translation:** text is sent only when you click the bubble, press the hotkey or click **Convert**, and only to the engine you selected in Settings:
  - **Ollama:** your own local server; nothing leaves your computer.
  - **OpenRouter:** openrouter.ai ([privacy policy](https://openrouter.ai/privacy)).
  - **OpenAI-compatible:** the base URL you entered.
  - **Google:** translate.googleapis.com ([privacy policy](https://policies.google.com/privacy)).
  - **Auto:** tries Ollama, then OpenRouter if you're signed in, then Google. To prevent fallback to cloud services, select **Ollama** instead of **Auto**.
- **Sign-in:** OpenRouter sign-in opens openrouter.ai in your browser at your request.
- **No telemetry:** no analytics, crash reporting, or update checks.
- **Local storage:** settings are stored in `%APPDATA%\tech.openforge.translatebubble\`, and API keys and tokens are stored in Windows Credential Manager.

## Uninstalling
Use **Windows Settings → Apps → Installed apps → Translate Bubble → Uninstall**, or run `uninstall.exe` in `%LOCALAPPDATA%\Translate Bubble\`.
