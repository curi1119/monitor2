# monitor2

Windows専用のCPU・メモリ・NVIDIA GPUモニター。
軽量・低メモリを優先してRustで開発します。

現在は開発環境の準備段階です。src/main.rsはビルド確認用の最小プログラムです。

## 開発環境

- Windows x64
- Rust stable (x86_64-pc-windows-msvc)、Cargo、rustfmt、Clippy
- Visual Studio 2022 Build Tools: C++ build tools、Windows SDK

Rustは公式のrustupでインストールします: https://rust-lang.org/tools/install/
インストール後はターミナルを開き直し、rustc --versionとcargo --versionを確認します。

## コマンド

```powershell
cargo run
cargo build --release
cargo fmt --check
cargo clippy -- -D warnings
```

## 次の作業

1. CPU・RAM・GPUの情報取得を試作し、実機で値を検証する。
2. 取得処理をUIから分離し、Win32の常駐画面を追加する。
3. 現行版とのCPU時間・メモリ比較と長時間動作を確認する。

元プロジェクト: D:\home\monitor
引き継ぎ: HANDOFF.md
