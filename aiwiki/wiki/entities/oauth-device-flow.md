---
title: "OAuth Device Flow"
entity_type: "technology"
type: entity
generated: "2026-04-19T15:28:18.510485239+00:00"
---

# OAuth Device Flow

**Type:** technology

### From: copilot

The OAuth 2.0 Device Authorization Grant, commonly called the Device Flow, is an authentication protocol specifically designed for devices with limited input capabilities or browserless environments. GitHub implements this flow for Copilot authentication, and the ragent-core provider includes complete support for initiating and polling this flow. The protocol is defined in RFC 8628 and solves the fundamental problem of how to authenticate a user on a device that cannot easily receive their credentials—such as a terminal application, embedded system, or in this case, a headless AI agent runtime.

The Device Flow operates through a distinct two-phase process. First, the client application makes a request to GitHub's device authorization endpoint with its client identifier, receiving in response a device code, user code, and verification URI. The user code is displayed to the user, who must then open the verification URI on a secondary device (typically a phone or computer with a browser) and enter the code. Meanwhile, the client application enters a polling loop, repeatedly requesting access tokens from GitHub's token endpoint using the device code. The server responds with `authorization_pending` until the user completes the browser step, at which point it returns the access token or an error if the code expired or was denied.

The implementation in `copilot.rs` exposes this flow through `start_copilot_device_flow()` and `poll_copilot_device_flow()` functions. The `DeviceFlowStart` struct captures the response from the initial request, including the `user_code` for display and `verification_uri` for user instruction. The polling implementation handles the expected transient states of the Device Flow, with appropriate backoff and timeout handling. Critically, the implementation uses GitHub's specific client ID `Iv1.b507a08c87ecfe98`, which is the same identifier used by official Copilot integrations including VS Code and Neovim. This reuse is significant because it means tokens obtained through this flow carry the same permissions and are subject to the same policies as tokens from official clients.

## Sources

- [copilot](../sources/copilot.md)
