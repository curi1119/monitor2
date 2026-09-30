# ビルドとリリース

`.github/workflows/ci.yml`: pushとpull_requestでWindows 2022上の整形、Clippy、テスト、releaseビルドを実行します。

`.github/workflows/release.yml`: GitHub ActionsのReleaseでRun workflowを選択し、Cargo.tomlと同じバージョン（例0.1.0）を入力します。検証後、EXE、利用方法、MITライセンス、外部素材の条件を含むZIPとSHA256を作り、v付きタグとGitHub Releaseを作成します。draftをオンにすると下書きになります。既存タグの上書きは行いません。ワークフローをデフォルトブランチに配置してから手動実行してください。

ローカルでの配布物作成:

```powershell
cargo build --locked --release
./scripts/package-release.ps1 -Version 0.1.0
```

出力はtarget/dist/。このリポジトリはまだリモート未設定のため、GitHub上でのワークフロー実行は未検証です。

参考: [workflow_dispatch](https://docs.github.com/en/actions/reference/workflows-and-actions/events-that-trigger-workflows#workflow_dispatch)、[gh release create](https://cli.github.com/manual/gh_release_create)。
