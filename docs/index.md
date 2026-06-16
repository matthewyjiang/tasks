---
layout: home

hero:
  name: tsk
  text: Local-first encrypted tasks
  tagline: A lightweight task manager with offline-first clients and zero-knowledge sync.
  actions:
    - theme: brand
      text: Get started with the CLI
      link: /cli
    - theme: alt
      text: Architecture
      link: /architecture

features:
  - title: Local-first
    details: Clients keep a local SQLite task database and continue working offline.
  - title: End-to-end encrypted
    details: Task blobs are encrypted on clients before they are synced through the server.
  - title: Shared core
    details: Rust core APIs back the CLI, Linux app, and iOS app through thin platform adapters.
---
